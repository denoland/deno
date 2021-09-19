// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno::colors;
use deno::flags;
use deno::logger;
use deno::standalone;
use deno::tokio_util;
use deno::unix_util;
use deno::AnyError;
use std::io::Write;

fn setup_exit_process_panic_hook() {
  // tokio does not exit the process when a task panics, so we
  // define a custom panic hook to implement this behaviour
  let orig_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |panic_info| {
    orig_hook(panic_info);
    std::process::exit(1);
  }));
}

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      eprintln!("{}: {:?}", colors::red_bold("error"), error);
      std::process::exit(1);
    }
  }
}

pub fn main() {
  setup_exit_process_panic_hook();
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10
  unix_util::raise_fd_limit();

  let args: Vec<String> = std::env::args().collect();
  let standalone_res = match standalone::extract_standalone(args.clone()) {
    Ok(Some((metadata, bundle))) => {
      tokio_util::run_basic(standalone::run(bundle, metadata))
    }
    Ok(None) => Ok(()),
    Err(err) => Err(err),
  };
  if let Err(err) = standalone_res {
    eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
    std::process::exit(1);
  }

  let flags = match flags::flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. })
      if err.kind == clap::ErrorKind::HelpDisplayed
        || err.kind == clap::ErrorKind::VersionDisplayed =>
    {
      err.write_to(&mut std::io::stdout()).unwrap();
      std::io::stdout().write_all(b"\n").unwrap();
      std::process::exit(0);
    }
    Err(err) => unwrap_or_exit(Err(AnyError::from(err))),
  };
  if !flags.v8_flags.is_empty() {
    deno::init_v8_flags(&*flags.v8_flags);
  }

  logger::init(flags.log_level);

  unwrap_or_exit(tokio_util::run_basic(deno::get_subcommand(flags)));
}
