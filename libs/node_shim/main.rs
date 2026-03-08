// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::env;
use std::process::Stdio;
use std::process::{self};

use node_shim::TranslateOptions;
use node_shim::translate_to_deno_args;

fn main() {
  let args = env::args().skip(1).collect::<Vec<String>>();

  let parsed_args = match node_shim::parse_args(args) {
    Ok(parsed_args) => parsed_args,
    Err(e) => {
      if e.len() == 1 {
        eprintln!("Error: {}", e[0]);
      } else if e.len() > 1 {
        eprintln!("Errors: {}", e.join(", "));
      }
      process::exit(1);
    }
  };

  // Handle --help specially for CLI
  if parsed_args.options.print_help {
    println!(
      "This is a shim that translates Node CLI arguments to Deno CLI arguments."
    );
    println!(
      "Use exactly like you would use Node.js, but it will run with Deno."
    );
    process::exit(0);
  }

  let options = TranslateOptions::for_node_cli();
  let result = translate_to_deno_args(parsed_args, &options);

  // Set DENO_TLS_CA_STORE if needed
  if result.use_system_ca {
    // SAFETY: This is set before any threads are spawned.
    unsafe { std::env::set_var("DENO_TLS_CA_STORE", "system") };
  }

  let mut deno_args = result.deno_args;

  // Handle entrypoint resolution for run commands
  if deno_args.len() >= 3 && deno_args.get(1) == Some(&"run".to_string()) {
    // Find the entrypoint (first non-flag arg after "run")
    let mut entrypoint_idx = None;
    for (i, arg) in deno_args.iter().enumerate().skip(2) {
      if !arg.starts_with('-') && !arg.starts_with("--") {
        entrypoint_idx = Some(i);
        break;
      }
    }

    if let Some(idx) = entrypoint_idx {
      let entrypoint = &deno_args[idx];
      let resolved = resolve_entrypoint(entrypoint);
      deno_args[idx] = resolved;
    }
  }

  if std::env::var("NODE_SHIM_DEBUG").is_ok() {
    eprintln!("deno {:?}", deno_args);
    process::exit(0);
  }

  // Execute deno with the translated arguments
  #[cfg(unix)]
  {
    let err = exec::execvp("deno", &deno_args);
    eprintln!("Failed to execute deno: {}", err);
    process::exit(1);
  }

  #[cfg(not(unix))]
  {
    let status = process::Command::new("deno")
      .args(&deno_args[1..])
      .status()
      .expect("Failed to execute deno");
    process::exit(status.code().unwrap_or(1));
  }
}

fn resolve_entrypoint(entrypoint: &str) -> String {
  let cwd = env::current_dir().unwrap();
  // If the entrypoint is either an absolute path, or a relative path that exists,
  // return it as is.
  if cwd.join(entrypoint).symlink_metadata().is_ok() {
    return entrypoint.to_string();
  }

  let url = url::Url::from_file_path(cwd.join("$file.js")).unwrap();

  // Otherwise, shell out to `deno` to try to resolve the entrypoint.
  let output = process::Command::new("deno")
    .arg("eval")
    .arg("--no-config")
    .arg(include_str!("./resolve.js"))
    .arg(url.to_string())
    .arg(format!("./{}", entrypoint))
    .env_clear()
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .output()
    .expect("Failed to execute deno resolve script");
  if !output.status.success() {
    std::process::exit(output.status.code().unwrap_or(1));
  }
  String::from_utf8(output.stdout)
    .expect("Failed to parse deno resolve output")
    .trim()
    .to_string()
}
