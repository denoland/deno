// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use std::collections::HashMap;
use std::convert::From;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::SystemTime;
use test_util::PathRef;

include!("../util/time.rs");

mod http;
mod lsp;

fn read_json(filename: &Path) -> Result<Value> {
  let f = fs::File::open(filename)?;
  Ok(serde_json::from_reader(f)?)
}

fn write_json(filename: &Path, value: &Value) -> Result<()> {
  let f = fs::File::create(filename)?;
  serde_json::to_writer(f, value)?;
  Ok(())
}

/// The list of the tuples of the benchmark name, arguments and return code
const EXEC_TIME_BENCHMARKS: &[(&str, &[&str], Option<i32>)] = &[
  // we need to run the cold_* benchmarks before the _warm_ ones as they ensure
  // the cache is properly populated, instead of other tests possibly
  // invalidating that cache.
  (
    "cold_hello",
    &["run", "--reload", "tests/testdata/run/002_hello.ts"],
    None,
  ),
  (
    "cold_relative_import",
    &[
      "run",
      "--reload",
      "tests/testdata/run/003_relative_import.ts",
    ],
    None,
  ),
  ("hello", &["run", "tests/testdata/run/002_hello.ts"], None),
  (
    "relative_import",
    &["run", "tests/testdata/run/003_relative_import.ts"],
    None,
  ),
  (
    "error_001",
    &["run", "tests/testdata/run/error_001.ts"],
    Some(1),
  ),
  (
    "no_check_hello",
    &[
      "run",
      "--reload",
      "--no-check",
      "tests/testdata/run/002_hello.ts",
    ],
    None,
  ),
  (
    "workers_startup",
    &[
      "run",
      "--allow-read",
      "tests/testdata/workers/bench_startup.ts",
    ],
    None,
  ),
  (
    "workers_round_robin",
    &[
      "run",
      "--allow-read",
      "tests/testdata/workers/bench_round_robin.ts",
    ],
    None,
  ),
  (
    "workers_large_message",
    &[
      "run",
      "--allow-read",
      "tests/testdata/workers/bench_large_message.ts",
    ],
    None,
  ),
  (
    "text_decoder",
    &["run", "tests/testdata/benches/text_decoder_perf.js"],
    None,
  ),
  (
    "text_encoder",
    &["run", "tests/testdata/benches/text_encoder_perf.js"],
    None,
  ),
  (
    "text_encoder_into",
    &["run", "tests/testdata/benches/text_encoder_into_perf.js"],
    None,
  ),
  (
    "response_string",
    &["run", "tests/testdata/benches/response_string_perf.js"],
    None,
  ),
  (
    "check",
    &[
      "check",
      "--reload",
      "--unstable",
      "tests/util/std/http/file_server_test.ts",
    ],
    None,
  ),
  (
    "no_check",
    &[
      "cache",
      "--reload",
      "--no-check",
      "--unstable",
      "tests/util/std/http/file_server_test.ts",
    ],
    None,
  ),
  (
    "bundle",
    &[
      "bundle",
      "--unstable",
      "tests/util/std/http/file_server_test.ts",
    ],
    None,
  ),
  (
    "bundle_no_check",
    &[
      "bundle",
      "--no-check",
      "--unstable",
      "tests/util/std/http/file_server_test.ts",
    ],
    None,
  ),
];

const RESULT_KEYS: &[&str] =
  &["mean", "stddev", "user", "system", "min", "max"];
fn run_exec_time(
  deno_exe: &Path,
  target_dir: &PathRef,
) -> Result<HashMap<String, HashMap<String, f64>>> {
  let hyperfine_exe = test_util::prebuilt_tool_path("hyperfine").to_string();

  let benchmark_file = target_dir.join("hyperfine_results.json");
  let benchmark_file_str = benchmark_file.to_string();

  let mut command = [
    hyperfine_exe.as_str(),
    "--export-json",
    benchmark_file_str.as_str(),
    "--warmup",
    "3",
  ]
  .iter()
  .map(|s| s.to_string())
  .collect::<Vec<_>>();

  for (_, args, return_code) in EXEC_TIME_BENCHMARKS {
    let ret_code_test = if let Some(code) = return_code {
      // Bash test which asserts the return code value of the previous command
      // $? contains the return code of the previous command
      format!("; test $? -eq {code}")
    } else {
      "".to_string()
    };
    command.push(format!(
      "{} {} {}",
      deno_exe.to_str().unwrap(),
      args.join(" "),
      ret_code_test
    ));
  }

  test_util::run(
    &command.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
    None,
    None,
    None,
    true,
  );

  let mut results = HashMap::<String, HashMap<String, f64>>::new();
  let hyperfine_results = read_json(benchmark_file.as_path())?;
  for ((name, _, _), data) in EXEC_TIME_BENCHMARKS.iter().zip(
    hyperfine_results
      .as_object()
      .unwrap()
      .get("results")
      .unwrap()
      .as_array()
      .unwrap(),
  ) {
    let data = data.as_object().unwrap().clone();
    results.insert(
      name.to_string(),
      data
        .into_iter()
        .filter(|(key, _)| RESULT_KEYS.contains(&key.as_str()))
        .map(|(key, val)| (key, val.as_f64().unwrap()))
        .collect(),
    );
  }

  Ok(results)
}

fn rlib_size(target_dir: &std::path::Path, prefix: &str) -> i64 {
  let mut size = 0;
  let mut seen = std::collections::HashSet::new();
  for entry in std::fs::read_dir(target_dir.join("deps")).unwrap() {
    let entry = entry.unwrap();
    let os_str = entry.file_name();
    let name = os_str.to_str().unwrap();
    if name.starts_with(prefix) && name.ends_with(".rlib") {
      let start = name.split('-').next().unwrap().to_string();
      if seen.contains(&start) {
        println!("skip {name}");
      } else {
        seen.insert(start);
        size += entry.metadata().unwrap().len();
        println!("check size {name} {size}");
      }
    }
  }
  assert!(size > 0);
  size as i64
}

const BINARY_TARGET_FILES: &[&str] = &[
  "CLI_SNAPSHOT.bin",
  "RUNTIME_SNAPSHOT.bin",
  "COMPILER_SNAPSHOT.bin",
];
fn get_binary_sizes(target_dir: &Path) -> Result<HashMap<String, i64>> {
  let mut sizes = HashMap::<String, i64>::new();
  let mut mtimes = HashMap::<String, SystemTime>::new();

  sizes.insert(
    "deno".to_string(),
    test_util::deno_exe_path().as_path().metadata()?.len() as i64,
  );

  // add up size for everything in target/release/deps/libswc*
  let swc_size = rlib_size(target_dir, "libswc");
  println!("swc {swc_size} bytes");
  sizes.insert("swc_rlib".to_string(), swc_size);

  let v8_size = rlib_size(target_dir, "libv8");
  println!("v8 {v8_size} bytes");
  sizes.insert("rusty_v8_rlib".to_string(), v8_size);

  // Because cargo's OUT_DIR is not predictable, search the build tree for
  // snapshot related files.
  for file in walkdir::WalkDir::new(target_dir) {
    let file = match file {
      Ok(file) => file,
      Err(_) => continue,
    };
    let filename = file.file_name().to_str().unwrap().to_string();

    if !BINARY_TARGET_FILES.contains(&filename.as_str()) {
      continue;
    }

    let meta = file.metadata()?;
    let file_mtime = meta.modified()?;

    // If multiple copies of a file are found, use the most recent one.
    if let Some(stored_mtime) = mtimes.get(&filename) {
      if *stored_mtime > file_mtime {
        continue;
      }
    }

    mtimes.insert(filename.clone(), file_mtime);
    sizes.insert(filename, meta.len() as i64);
  }

  Ok(sizes)
}

const BUNDLES: &[(&str, &str)] = &[
  ("file_server", "./tests/util/std/http/file_server.ts"),
  ("welcome", "./tests/testdata/welcome.ts"),
];
fn bundle_benchmark(deno_exe: &Path) -> Result<HashMap<String, i64>> {
  let mut sizes = HashMap::<String, i64>::new();

  for (name, url) in BUNDLES {
    let path = format!("{name}.bundle.js");
    test_util::run(
      &[
        deno_exe.to_str().unwrap(),
        "bundle",
        "--unstable",
        url,
        &path,
      ],
      None,
      None,
      None,
      true,
    );

    let file = PathBuf::from(path);
    assert!(file.is_file());
    sizes.insert(name.to_string(), file.metadata()?.len() as i64);
    let _ = fs::remove_file(file);
  }

  Ok(sizes)
}

fn run_max_mem_benchmark(deno_exe: &Path) -> Result<HashMap<String, i64>> {
  let mut results = HashMap::<String, i64>::new();

  for (name, args, return_code) in EXEC_TIME_BENCHMARKS {
    let proc = Command::new("time")
      .args(["-v", deno_exe.to_str().unwrap()])
      .args(args.iter())
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()?;

    let proc_result = proc.wait_with_output()?;
    if let Some(code) = return_code {
      assert_eq!(proc_result.status.code().unwrap(), *code);
    }
    let out = String::from_utf8(proc_result.stderr)?;

    results.insert(
      name.to_string(),
      test_util::parse_max_mem(&out).unwrap() as i64,
    );
  }

  Ok(results)
}

fn cargo_deps() -> usize {
  let cargo_lock = test_util::root_path().join("Cargo.lock");
  let mut count = 0;
  let file = std::fs::File::open(cargo_lock).unwrap();
  use std::io::BufRead;
  for line in std::io::BufReader::new(file).lines() {
    if line.unwrap().starts_with("[[package]]") {
      count += 1
    }
  }
  println!("cargo_deps {count}");
  assert!(count > 10); // Sanity check.
  count
}

// TODO(@littledivy): Remove this, denoland/benchmark_data is deprecated.
#[derive(Default, serde::Serialize)]
struct BenchResult {
  created_at: String,
  sha1: String,

  // TODO(ry) The "benchmark" benchmark should actually be called "exec_time".
  // When this is changed, the historical data in gh-pages branch needs to be
  // changed too.
  benchmark: HashMap<String, HashMap<String, f64>>,
  binary_size: HashMap<String, i64>,
  bundle_size: HashMap<String, i64>,
  cargo_deps: usize,
  max_latency: HashMap<String, f64>,
  max_memory: HashMap<String, i64>,
  lsp_exec_time: HashMap<String, i64>,
  req_per_sec: HashMap<String, i64>,
  syscall_count: HashMap<String, i64>,
  thread_count: HashMap<String, i64>,
}

#[tokio::main]
async fn main() -> Result<()> {
  let mut args = env::args();

  let mut benchmarks = vec![
    "bundle",
    "exec_time",
    "binary_size",
    "cargo_deps",
    "lsp",
    "http",
    "strace",
    "mem_usage",
  ];

  let mut found_bench = false;
  let filter = args.nth(1);
  if let Some(filter) = filter {
    if filter != "--bench" {
      benchmarks.retain(|s| s == &filter);
    } else {
      found_bench = true;
    }
  }

  if !found_bench && !args.any(|s| s == "--bench") {
    return Ok(());
  }

  println!("Starting Deno benchmark");

  let target_dir = test_util::target_dir();
  let deno_exe = if let Ok(p) = std::env::var("DENO_BENCH_EXE") {
    PathBuf::from(p)
  } else {
    test_util::deno_exe_path().to_path_buf()
  };
  env::set_current_dir(test_util::root_path())?;

  let mut new_data = BenchResult {
    created_at: utc_now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    sha1: test_util::run_collect(
      &["git", "rev-parse", "HEAD"],
      None,
      None,
      None,
      true,
    )
    .0
    .trim()
    .to_string(),
    ..Default::default()
  };

  if benchmarks.contains(&"bundle") {
    let bundle_size = bundle_benchmark(&deno_exe)?;
    new_data.bundle_size = bundle_size;
  }

  if benchmarks.contains(&"exec_time") {
    let exec_times = run_exec_time(&deno_exe, &target_dir)?;
    new_data.benchmark = exec_times;
  }

  if benchmarks.contains(&"binary_size") {
    let binary_sizes = get_binary_sizes(target_dir.as_path())?;
    new_data.binary_size = binary_sizes;
  }

  if benchmarks.contains(&"cargo_deps") {
    let cargo_deps = cargo_deps();
    new_data.cargo_deps = cargo_deps;
  }

  if benchmarks.contains(&"lsp") {
    let lsp_exec_times = lsp::benchmarks(&deno_exe);
    new_data.lsp_exec_time = lsp_exec_times;
  }

  if benchmarks.contains(&"http") && cfg!(not(target_os = "windows")) {
    let stats = http::benchmark(target_dir.as_path())?;
    let req_per_sec = stats
      .iter()
      .map(|(name, result)| (name.clone(), result.requests as i64))
      .collect();
    new_data.req_per_sec = req_per_sec;
    let max_latency = stats
      .iter()
      .map(|(name, result)| (name.clone(), result.latency))
      .collect();

    new_data.max_latency = max_latency;
  }

  if cfg!(target_os = "linux") && benchmarks.contains(&"strace") {
    use std::io::Read;

    let mut thread_count = HashMap::<String, i64>::new();
    let mut syscall_count = HashMap::<String, i64>::new();

    for (name, args, expected_exit_code) in EXEC_TIME_BENCHMARKS {
      let mut file = tempfile::NamedTempFile::new()?;

      let exit_status = Command::new("strace")
        .args([
          "-c",
          "-f",
          "-o",
          file.path().to_str().unwrap(),
          deno_exe.to_str().unwrap(),
        ])
        .args(args.iter())
        .stdout(Stdio::null())
        .env("LC_NUMERIC", "C")
        .spawn()?
        .wait()?;
      let expected_exit_code = expected_exit_code.unwrap_or(0);
      assert_eq!(exit_status.code(), Some(expected_exit_code));

      let mut output = String::new();
      file.as_file_mut().read_to_string(&mut output)?;

      let strace_result = test_util::parse_strace_output(&output);
      let clone =
        strace_result
          .get("clone")
          .map(|d| d.calls)
          .unwrap_or_else(|| {
            strace_result.get("clone3").map(|d| d.calls).unwrap_or(0)
          })
          + 1;
      let total = strace_result.get("total").unwrap().calls;
      thread_count.insert(name.to_string(), clone as i64);
      syscall_count.insert(name.to_string(), total as i64);
    }

    new_data.thread_count = thread_count;
    new_data.syscall_count = syscall_count;
  }

  if benchmarks.contains(&"mem_usage") {
    let max_memory = run_max_mem_benchmark(&deno_exe)?;
    new_data.max_memory = max_memory;
  }

  write_json(
    target_dir.join("bench.json").as_path(),
    &serde_json::to_value(&new_data)?,
  )?;

  Ok(())
}

pub type Result<T> = std::result::Result<T, AnyError>;
