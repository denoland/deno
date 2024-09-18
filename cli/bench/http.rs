// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use super::Result;

pub use test_util::parse_wrk_output;
pub use test_util::WrkOutput as HttpBenchmarkResult;
// Some of the benchmarks in this file have been renamed. In case the history
// somehow gets messed up:
//   "node_http" was once called "node"
//   "deno_tcp" was once called "deno"
//   "deno_http" was once called "deno_net_http"

const DURATION: &str = "10s";

pub fn benchmark(
  target_path: &Path,
) -> Result<HashMap<String, HttpBenchmarkResult>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_string();

  let hyper_hello_exe = target_path.join("test_server");
  let hyper_hello_exe = hyper_hello_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let http_dir = manifest_dir.join("bench").join("http");
  for entry in std::fs::read_dir(&http_dir)? {
    let entry = entry?;
    let pathbuf = entry.path();
    let path = pathbuf.to_str().unwrap();
    if path.ends_with(".lua") {
      continue;
    }
    let file_stem = pathbuf.file_stem().unwrap().to_str().unwrap();

    let lua_script = http_dir.join(format!("{file_stem}.lua"));
    let mut maybe_lua = None;
    if lua_script.exists() {
      maybe_lua = Some(lua_script.to_str().unwrap());
    }

    let port = get_port();
    // deno run -A --unstable-net <path> <addr>
    res.insert(
      file_stem.to_string(),
      run(
        &[
          deno_exe.as_str(),
          "run",
          "--allow-all",
          "--unstable-net",
          "--enable-testing-features-do-not-use",
          path,
          &server_addr(port),
        ],
        port,
        None,
        None,
        maybe_lua,
      )?,
    );
  }

  res.insert("hyper".to_string(), hyper_http(hyper_hello_exe)?);

  Ok(res)
}

fn run(
  server_cmd: &[&str],
  port: u16,
  env: Option<Vec<(String, String)>>,
  origin_cmd: Option<&[&str]>,
  lua_script: Option<&str>,
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

  // Wait for server to wake up.
  let now = Instant::now();
  let addr = format!("127.0.0.1:{port}");
  while now.elapsed().as_secs() < 30 {
    if TcpStream::connect(&addr).is_ok() {
      break;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
  TcpStream::connect(&addr).expect("Failed to connect to server in time");
  println!("Server took {} ms to start", now.elapsed().as_millis());

  let wrk = test_util::prebuilt_tool_path("wrk");
  assert!(wrk.is_file());

  let addr = format!("http://{addr}/");
  let wrk = wrk.to_string();
  let mut wrk_cmd = vec![wrk.as_str(), "-d", DURATION, "--latency", &addr];

  if let Some(lua_script) = lua_script {
    wrk_cmd.push("-s");
    wrk_cmd.push(lua_script);
  }

  println!("{}", wrk_cmd.join(" "));
  let output = test_util::run_collect(&wrk_cmd, None, None, None, true).0;

  std::thread::sleep(Duration::from_secs(1)); // wait to capture failure. TODO racy.

  println!("{output}");
  assert!(
    server.try_wait()?.map(|s| s.success()).unwrap_or(true),
    "server ended with error"
  );

  server.kill()?;
  if let Some(mut origin) = origin {
    origin.kill()?;
  }

  Ok(parse_wrk_output(&output))
}

static NEXT_PORT: AtomicU16 = AtomicU16::new(4544);
pub(crate) fn get_port() -> u16 {
  let p = NEXT_PORT.load(Ordering::SeqCst);
  NEXT_PORT.store(p.wrapping_add(1), Ordering::SeqCst);
  p
}

fn server_addr(port: u16) -> String {
  format!("0.0.0.0:{port}")
}

fn hyper_http(exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing RUST hyper");
  run(&[exe, &port.to_string()], port, None, None, None)
}
