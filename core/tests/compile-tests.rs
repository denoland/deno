use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

use anyhow::bail;
use anyhow::Error;

struct CompileTestFeatures {
  build_time_includes: bool,
  snapshot: bool,
}

fn run_compile_test(features: CompileTestFeatures) -> Result<Output, Error> {
  let mut command = Command::new(match std::env::var_os("CARGO") {
    Some(cargo_path) => PathBuf::from(cargo_path),
    None => PathBuf::from("cargo"),
  });
  command.arg("run");
  if features.build_time_includes || features.snapshot {
    command.arg("--features");
    let mut feature_vec = vec![];
    if features.build_time_includes {
      feature_vec.push("build-time-includes");
    }
    if features.snapshot {
      feature_vec.push("snapshot");
    }
    command.arg(feature_vec.join(","));
  }
  command.current_dir({
    let mut cwd_path = match std::env::var_os("CARGO_MANIFEST_DIR") {
      Some(path) => PathBuf::from(path),
      None => panic!("Invalid envvar CARGO_MANIFEST_DIR."),
    };
    cwd_path.push("tests");
    cwd_path.push("compile-test-crate");
    cwd_path
  });
  Ok(command.output()?)
}

fn panic_based_on_output_status(output: Output) -> Result<(), Error> {
  let is_fail = !output.status.success() || output.stdout != "Hi!".as_bytes();

  if is_fail {
    println!("-----------------------------");
    println!("STDOUT:");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("-----------------------------");
    println!("STDERR:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    println!("-----------------------------");

    if !output.status.success() {
      match output.status.code() {
        Some(code) => bail!("Compilation failed with status code {}", code),
        None => bail!("Process terminated by a signal."),
      }
    } else {
      bail!("Expected a stdout of 'Hi!'.");
    }
  }

  Ok(())
}

#[test]
fn runtime_without_feature() -> Result<(), Error> {
  let output = run_compile_test(CompileTestFeatures {
    build_time_includes: false,
    snapshot: false,
  })?;
  panic_based_on_output_status(output)?;

  Ok(())
}

#[test]
fn compile_time_without_feature() -> Result<(), Error> {
  let output = run_compile_test(CompileTestFeatures {
    build_time_includes: false,
    snapshot: true,
  })?;
  panic_based_on_output_status(output)?;

  Ok(())
}

#[test]
fn compile_time_with_feature() -> Result<(), Error> {
  let output = run_compile_test(CompileTestFeatures {
    build_time_includes: true,
    snapshot: true,
  })?;
  panic_based_on_output_status(output)?;

  Ok(())
}

#[test]
fn runtime_with_feature() -> Result<(), Error> {
  let output = run_compile_test(CompileTestFeatures {
    build_time_includes: true,
    snapshot: false,
  })?;

  if output.status.success() {
    println!("-----------------------------");
    println!("STDOUT:");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("-----------------------------");
    println!("STDERR:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    println!("-----------------------------");
    bail!("Running succeeded when it shouldn't have.");
  }

  if !output.stdout.is_empty() {
    println!("-----------------------------");
    println!("STDOUT:");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("-----------------------------");
    println!("STDERR:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    println!("-----------------------------");
    bail!("Expected stdout to be empty.");
  }

  let expected_stderr = concat!(
    "thread 'main' panicked at 'The build-time-includes feature on deno_core ",
    "should only be used when building snapshots in a build script.', ",
    "core/ops_builtin.rs:"
  );

  if !String::from_utf8_lossy(&output.stderr).contains(expected_stderr) {
    println!("-----------------------------");
    println!("STDERR:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    println!("-----------------------------");
    bail!("Expected the program to panic with the right message.");
  }

  Ok(())
}
