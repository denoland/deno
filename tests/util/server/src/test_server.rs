// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

fn main() {
  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();
  setup_panic_hook();
  test_server::servers::run_all_servers();
}

fn setup_panic_hook() {
  // Tokio does not exit the process when a task panics, so we define a custom
  // panic hook to implement this behaviour.
  let orig_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |panic_info| {
    eprintln!("\n============================================================");
    eprintln!("Test server panicked!\n");
    orig_hook(panic_info);
    std::process::exit(1);
  }));
}
