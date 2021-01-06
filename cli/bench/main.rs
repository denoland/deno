// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::{self, map::Map, Number, Value};
use std::{
  convert::From,
  env, fs,
  path::PathBuf,
  process::{Command, Stdio},
};

mod http;
mod throughput;

fn read_json(filename: &str) -> Result<Value> {
  let f = fs::File::open(filename)?;
  Ok(serde_json::from_reader(f)?)
}

fn write_json(filename: &str, value: &Value) -> Result<()> {
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
    &["run", "--reload", "cli/tests/002_hello.ts"],
    None,
  ),
  (
    "cold_relative_import",
    &["run", "--reload", "cli/tests/003_relative_import.ts"],
    None,
  ),
  ("hello", &["run", "cli/tests/002_hello.ts"], None),
  (
    "relative_import",
    &["run", "cli/tests/003_relative_import.ts"],
    None,
  ),
  ("error_001", &["run", "cli/tests/error_001.ts"], Some(1)),
  (
    "no_check_hello",
    &["run", "--reload", "--no-check", "cli/tests/002_hello.ts"],
    None,
  ),
  (
    "workers_startup",
    &["run", "--allow-read", "cli/tests/workers_startup_bench.ts"],
    None,
  ),
  (
    "workers_round_robin",
    &[
      "run",
      "--allow-read",
      "cli/tests/workers_round_robin_bench.ts",
    ],
    None,
  ),
  (
    "text_decoder",
    &["run", "cli/tests/text_decoder_perf.js"],
    None,
  ),
  (
    "text_encoder",
    &["run", "cli/tests/text_encoder_perf.js"],
    None,
  ),
  (
    "check",
    &["cache", "--reload", "std/examples/chat/server_test.ts"],
    None,
  ),
  (
    "no_check",
    &[
      "cache",
      "--reload",
      "--no-check",
      "std/examples/chat/server_test.ts",
    ],
    None,
  ),
  (
    "bundle",
    &["bundle", "std/examples/chat/server_test.ts"],
    None,
  ),
  (
    "bundle_no_check",
    &["bundle", "--no-check", "std/examples/chat/server_test.ts"],
    None,
  ),
];

const RESULT_KEYS: &[&str] =
  &["mean", "stddev", "user", "system", "min", "max"];
fn run_exec_time(deno_exe: &PathBuf, target_dir: &PathBuf) -> Result<Value> {
  let hyperfine_exe = test_util::prebuilt_tool_path("hyperfine");

  let benchmark_file = target_dir.join("hyperfine_results.json");
  let benchmark_file = benchmark_file.to_str().unwrap();

  let mut command = [
    hyperfine_exe.to_str().unwrap(),
    "--export-json",
    benchmark_file,
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
      format!("; test $? -eq {}", code)
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

  let mut results = Map::new();
  let hyperfine_results = read_json(benchmark_file)?;
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
      Value::Object(
        data
          .into_iter()
          .filter(|(key, _)| RESULT_KEYS.contains(&key.as_str()))
          .collect::<Map<String, Value>>(),
      ),
    );
  }

  Ok(Value::Object(results))
}

fn rlib_size(target_dir: &std::path::Path, prefix: &str) -> u64 {
  let mut size = 0;
  let mut seen = std::collections::HashSet::new();
  for entry in std::fs::read_dir(target_dir.join("deps")).unwrap() {
    let entry = entry.unwrap();
    let os_str = entry.file_name();
    let name = os_str.to_str().unwrap();
    if name.starts_with(prefix) && name.ends_with(".rlib") {
      let start = name.split('-').next().unwrap().to_string();
      if seen.contains(&start) {
        println!("skip {}", name);
      } else {
        seen.insert(start);
        size += entry.metadata().unwrap().len();
        println!("check size {} {}", name, size);
      }
    }
  }
  assert!(size > 0);
  size
}

const BINARY_TARGET_FILES: &[&str] =
  &["CLI_SNAPSHOT.bin", "COMPILER_SNAPSHOT.bin"];
fn get_binary_sizes(target_dir: &PathBuf) -> Result<Value> {
  let mut sizes = Map::new();
  let mut mtimes = std::collections::HashMap::new();

  sizes.insert(
    "deno".to_string(),
    Value::Number(Number::from(test_util::deno_exe_path().metadata()?.len())),
  );

  // add up size for denort
  sizes.insert(
    "denort".to_string(),
    Value::Number(Number::from(test_util::denort_exe_path().metadata()?.len()))
  );

  // add up size for everything in target/release/deps/libswc*
  let swc_size = rlib_size(&target_dir, "libswc");
  println!("swc {} bytes", swc_size);
  sizes.insert("swc_rlib".to_string(), Value::Number(swc_size.into()));

  let rusty_v8_size = rlib_size(&target_dir, "librusty_v8");
  println!("rusty_v8 {} bytes", rusty_v8_size);
  sizes.insert(
    "rusty_v8_rlib".to_string(),
    Value::Number(rusty_v8_size.into()),
  );

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
    sizes.insert(filename, Value::Number(Number::from(meta.len())));
  }

  Ok(Value::Object(sizes))
}

const BUNDLES: &[(&str, &str)] = &[
  ("file_server", "./std/http/file_server.ts"),
  ("gist", "./std/examples/gist.ts"),
];
fn bundle_benchmark(deno_exe: &PathBuf) -> Result<Value> {
  let mut sizes = Map::new();

  for (name, url) in BUNDLES {
    let path = format!("{}.bundle.js", name);
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
    sizes.insert(
      name.to_string(),
      Value::Number(Number::from(file.metadata()?.len())),
    );
    let _ = fs::remove_file(file);
  }

  Ok(Value::Object(sizes))
}

fn run_throughput(deno_exe: &PathBuf) -> Result<Value> {
  let mut m = Map::new();

  m.insert("100M_tcp".to_string(), throughput::tcp(deno_exe, 100)?);
  m.insert("100M_cat".to_string(), throughput::cat(deno_exe, 100)?);
  m.insert("10M_tcp".to_string(), throughput::tcp(deno_exe, 10)?);
  m.insert("10M_cat".to_string(), throughput::cat(deno_exe, 10)?);

  Ok(Value::Object(m))
}

fn run_http(
  target_dir: &PathBuf,
  new_data: &mut Map<String, Value>,
) -> Result<()> {
  let stats = http::benchmark(target_dir)?;

  new_data.insert(
    "req_per_sec".to_string(),
    Value::Object(
      stats
        .iter()
        .map(|(name, result)| {
          (name.clone(), Value::Number(Number::from(result.requests)))
        })
        .collect::<Map<String, Value>>(),
    ),
  );

  new_data.insert(
    "max_latency".to_string(),
    Value::Object(
      stats
        .iter()
        .map(|(name, result)| {
          (
            name.clone(),
            Value::Number(Number::from_f64(result.latency).unwrap()),
          )
        })
        .collect::<Map<String, Value>>(),
    ),
  );

  Ok(())
}

fn run_strace_benchmarks(
  deno_exe: &PathBuf,
  new_data: &mut Map<String, Value>,
) -> Result<()> {
  use std::io::Read;

  let mut thread_count = Map::new();
  let mut syscall_count = Map::new();

  for (name, args, _) in EXEC_TIME_BENCHMARKS {
    let mut file = tempfile::NamedTempFile::new()?;

    Command::new("strace")
      .args(&[
        "-c",
        "-f",
        "-o",
        file.path().to_str().unwrap(),
        deno_exe.to_str().unwrap(),
      ])
      .args(args.iter())
      .stdout(Stdio::inherit())
      .spawn()?
      .wait()?;

    let mut output = String::new();
    file.as_file_mut().read_to_string(&mut output)?;

    let strace_result = test_util::parse_strace_output(&output);
    thread_count.insert(
      name.to_string(),
      Value::Number(Number::from(
        strace_result.get("clone").map(|d| d.calls).unwrap_or(0) + 1,
      )),
    );
    syscall_count.insert(
      name.to_string(),
      Value::Number(Number::from(strace_result.get("total").unwrap().calls)),
    );
  }

  new_data.insert("thread_count".to_string(), Value::Object(thread_count));
  new_data.insert("syscall_count".to_string(), Value::Object(syscall_count));

  Ok(())
}

fn run_max_mem_benchmark(deno_exe: &PathBuf) -> Result<Value> {
  let mut results = Map::new();

  for (name, args, return_code) in EXEC_TIME_BENCHMARKS {
    let proc = Command::new("time")
      .args(&["-v", deno_exe.to_str().unwrap()])
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
      Value::Number(Number::from(test_util::parse_max_mem(&out).unwrap())),
    );
  }

  Ok(Value::Object(results))
}

/*
 TODO(SyrupThinker)
 Switch to the #[bench] attribute once
 it is stabilized.
 Before that the #[test] tests won't be run because
 we replace the harness with our own runner here.
*/
fn main() -> Result<()> {
  if env::args().find(|s| s == "--bench").is_none() {
    return Ok(());
  }

  println!("Starting Deno benchmark");

  let target_dir = test_util::target_dir();
  let deno_exe = test_util::deno_exe_path();

  env::set_current_dir(&test_util::root_path())?;

  let mut new_data: Map<String, Value> = Map::new();

  new_data.insert("binary_size".to_string(), get_binary_sizes(&target_dir)?);
  new_data.insert("bundle_size".to_string(), bundle_benchmark(&deno_exe)?);

  new_data.insert(
    "created_at".to_string(),
    Value::String(
      chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    ),
  );
  new_data.insert(
    "sha1".to_string(),
    Value::String(
      test_util::run_collect(
        &["git", "rev-parse", "HEAD"],
        None,
        None,
        None,
        true,
      )
      .0
      .trim()
      .to_string(),
    ),
  );

  // TODO(ry) The "benchmark" benchmark should actually be called "exec_time".
  // When this is changed, the historical data in gh-pages branch needs to be
  // changed too.
  new_data.insert(
    "benchmark".to_string(),
    run_exec_time(&deno_exe, &target_dir)?,
  );

  // Cannot run throughput benchmark on windows because they don't have nc or
  // pipe.
  if cfg!(not(target_os = "windows")) {
    new_data.insert("throughput".to_string(), run_throughput(&deno_exe)?);
    run_http(&target_dir, &mut new_data)?;
  }

  if cfg!(target_os = "linux") {
    run_strace_benchmarks(&deno_exe, &mut new_data)?;
    new_data
      .insert("max_memory".to_string(), run_max_mem_benchmark(&deno_exe)?);
  }

  println!("===== <BENCHMARK RESULTS>");
  serde_json::to_writer_pretty(std::io::stdout(), &new_data)?;
  println!("\n===== </BENCHMARK RESULTS>");

  if let Some(filename) = target_dir.join("bench.json").to_str() {
    write_json(filename, &Value::Object(new_data))?;
  } else {
    eprintln!("Cannot write bench.json, path is invalid");
  }

  Ok(())
}

#[derive(Debug)]
enum Error {
  Io(std::io::Error),
  Serde(serde_json::error::Error),
  FromUtf8(std::string::FromUtf8Error),
  Walkdir(walkdir::Error),
}

impl From<std::io::Error> for Error {
  fn from(ioe: std::io::Error) -> Self {
    Error::Io(ioe)
  }
}

impl From<serde_json::error::Error> for Error {
  fn from(sje: serde_json::error::Error) -> Self {
    Error::Serde(sje)
  }
}

impl From<std::string::FromUtf8Error> for Error {
  fn from(fue: std::string::FromUtf8Error) -> Self {
    Error::FromUtf8(fue)
  }
}

impl From<walkdir::Error> for Error {
  fn from(wde: walkdir::Error) -> Self {
    Error::Walkdir(wde)
  }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
