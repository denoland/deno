// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::print_stdout, reason = "CLI tool")]
#![allow(clippy::print_stderr, reason = "CLI tool")]
#![allow(clippy::disallowed_methods, reason = "CLI tool")]

use std::env;
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
  node_shim::resolve_run_entrypoint(
    std::path::Path::new("deno"),
    &mut deno_args,
  );

  if std::env::var("NODE_SHIM_DEBUG").is_ok() {
    eprintln!("deno {:?}", deno_args);
    process::exit(0);
  }

  // Execute deno with the translated arguments
  #[cfg(unix)]
  {
    use std::os::unix::process::CommandExt;
    let err = process::Command::new("deno")
      .arg0(&deno_args[0])
      .args(&deno_args[1..])
      .exec();
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
