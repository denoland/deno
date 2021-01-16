// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::Result;
use std::{
  collections::HashMap, path::PathBuf, process::Command, time::Duration,
};
pub use test_util::{parse_wrk_output, WrkOutput as HttpBenchmarkResult};

// Some of the benchmarks in this file have been renamed. In case the history
// somehow gets messed up:
//   "node_http" was once called "node"
//   "deno_tcp" was once called "deno"
//   "deno_http" was once called "deno_net_http"

const DURATION: &str = "20s";

pub(crate) fn benchmark(
  target_path: &PathBuf,
) -> Result<HashMap<String, HttpBenchmarkResult>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let hyper_hello_exe = target_path.join("test_server");
  let hyper_hello_exe = hyper_hello_exe.to_str().unwrap();

  let core_http_bin_ops_exe = target_path.join("examples/http_bench_bin_ops");
  let core_http_bin_ops_exe = core_http_bin_ops_exe.to_str().unwrap();

  let core_http_json_ops_exe = target_path.join("examples/http_bench_json_ops");
  let core_http_json_ops_exe = core_http_json_ops_exe.to_str().unwrap();

  let mut res = HashMap::new();

  // "deno_tcp" was once called "deno"
  res.insert("deno_tcp".to_string(), deno_tcp(deno_exe)?);
  // res.insert("deno_udp".to_string(), deno_udp(deno_exe)?);
  res.insert("deno_http".to_string(), deno_http(deno_exe)?);
  // TODO(ry) deno_proxy disabled to make fetch() standards compliant.
  // res.insert("deno_proxy".to_string(), deno_http_proxy(deno_exe) hyper_hello_exe))
  res.insert(
    "deno_proxy_tcp".to_string(),
    deno_tcp_proxy(deno_exe, hyper_hello_exe)?,
  );
  // "core_http_bin_ops" was once called "deno_core_single"
  // "core_http_bin_ops" was once called "deno_core_http_bench"
  res.insert(
    "core_http_bin_ops".to_string(),
    core_http_bin_ops(core_http_bin_ops_exe)?,
  );
  res.insert(
    "core_http_json_ops".to_string(),
    core_http_json_ops(core_http_json_ops_exe)?,
  );
  // "node_http" was once called "node"
  res.insert("node_http".to_string(), node_http()?);
  res.insert("node_proxy".to_string(), node_http_proxy(hyper_hello_exe)?);
  res.insert(
    "node_proxy_tcp".to_string(),
    node_tcp_proxy(hyper_hello_exe)?,
  );
  res.insert("node_tcp".to_string(), node_tcp()?);
  res.insert("hyper".to_string(), hyper_http(hyper_hello_exe)?);

  Ok(res)
}

fn run(
  server_cmd: &[&str],
  port: u16,
  env: Option<Vec<(String, String)>>,
  origin_cmd: Option<&[&str]>,
) -> Result<HttpBenchmarkResult> {
  // Wait for port 4544 to become available.
  // TODO Need to use SO_REUSEPORT with tokio::net::TcpListener.
  std::thread::sleep(Duration::from_secs(5));

  let mut origin = None;
  if let Some(cmd) = origin_cmd {
    let mut com = Command::new(cmd[0]);
    com.args(&cmd[1..]);
    if let Some(env) = env.clone() {
      com.envs(env);
    }
    origin = Some(com.spawn()?);
  };

  println!("{}", server_cmd.join(" "));
  let mut server = {
    let mut com = Command::new(server_cmd[0]);
    com.args(&server_cmd[1..]);
    if let Some(env) = env {
      com.envs(env);
    }
    com.spawn()?
  };

  std::thread::sleep(Duration::from_secs(5)); // wait for server to wake up. TODO racy.

  let wrk = test_util::prebuilt_tool_path("wrk");
  assert!(wrk.is_file());

  let wrk_cmd = &[
    wrk.to_str().unwrap(),
    "-d",
    DURATION,
    "--latency",
    &format!("http://127.0.0.1:{}/", port),
  ];
  println!("{}", wrk_cmd.join(" "));
  let output = test_util::run_collect(wrk_cmd, None, None, None, true).0;

  println!("{}", output);
  assert!(
    server.try_wait()?.map_or(true, |s| s.success()),
    "server ended with error"
  );

  server.kill()?;
  if let Some(mut origin) = origin {
    origin.kill()?;
  }

  Ok(parse_wrk_output(&output))
}

fn get_port() -> u16 {
  static mut NEXT_PORT: u16 = 4544;

  let port = unsafe { NEXT_PORT };

  unsafe {
    NEXT_PORT += 1;
  }

  port
}

fn server_addr(port: u16) -> String {
  format!("0.0.0.0:{}", port)
}

fn deno_tcp(deno_exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing DENO tcp.");
  run(
    &[
      deno_exe,
      "run",
      "--allow-net",
      "cli/bench/deno_tcp.ts",
      &server_addr(port),
    ],
    port,
    None,
    None,
  )
}

fn deno_tcp_proxy(
  deno_exe: &str,
  hyper_exe: &str,
) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  let origin_port = get_port();

  println!("http_proxy_benchmark testing DENO using net/tcp.");
  run(
    &[
      deno_exe,
      "run",
      "--allow-net",
      "--reload",
      "--unstable",
      "cli/bench/deno_tcp_proxy.ts",
      &server_addr(port),
      &server_addr(origin_port),
    ],
    port,
    None,
    Some(&[hyper_exe, &origin_port.to_string()]),
  )
}

fn deno_http(deno_exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing DENO using net/http.");
  run(
    &[
      deno_exe,
      "run",
      "--allow-net",
      "--reload",
      "--unstable",
      "std/http/bench.ts",
      &server_addr(port),
    ],
    port,
    None,
    None,
  )
}

#[allow(dead_code)]
fn deno_http_proxy(
  deno_exe: &str,
  hyper_exe: &str,
) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  let origin_port = get_port();

  println!("http_proxy_benchmark testing DENO using net/http.");
  run(
    &[
      deno_exe,
      "run",
      "--allow-net",
      "--reload",
      "--unstable",
      "cli/bench/deno_http_proxy.ts",
      &server_addr(port),
      &server_addr(origin_port),
    ],
    port,
    None,
    Some(&[hyper_exe, &origin_port.to_string()]),
  )
}

fn core_http_bin_ops(exe: &str) -> Result<HttpBenchmarkResult> {
  println!("http_benchmark testing CORE http_bench_bin_ops");
  run(&[exe], 4544, None, None)
}

fn core_http_json_ops(exe: &str) -> Result<HttpBenchmarkResult> {
  println!("http_benchmark testing CORE http_bench_json_ops");
  run(&[exe], 4544, None, None)
}

fn node_http() -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing NODE.");
  run(
    &["node", "cli/bench/node_http.js", &port.to_string()],
    port,
    None,
    None,
  )
}

fn node_http_proxy(hyper_exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  let origin_port = get_port();
  let origin_port = origin_port.to_string();

  println!("http_proxy_benchmark testing NODE.");
  run(
    &[
      "node",
      "cli/bench/node_http_proxy.js",
      &port.to_string(),
      &origin_port,
    ],
    port,
    None,
    Some(&[hyper_exe, &origin_port]),
  )
}

fn node_tcp_proxy(exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  let origin_port = get_port();
  let origin_port = origin_port.to_string();

  println!("http_proxy_benchmark testing NODE tcp.");
  run(
    &[
      "node",
      "cli/bench/node_tcp_proxy.js",
      &port.to_string(),
      &origin_port,
    ],
    port,
    None,
    Some(&[exe, &origin_port]),
  )
}

fn node_tcp() -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing node_tcp.js");
  run(
    &["node", "cli/bench/node_tcp.js", &port.to_string()],
    port,
    None,
    None,
  )
}

fn hyper_http(exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing RUST hyper");
  run(&[exe, &port.to_string()], port, None, None)
}
