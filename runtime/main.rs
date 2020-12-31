fn main() {
  #[cfg(windows)]
  deno_runtime::colors::enable_ansi(); // For Windows 10

  let args_raw: Vec<String> = std::env::args().collect();
  let args: Vec<String> = args_raw[1..].to_vec();
  if let Err(err) = deno_runtime::standalone::try_run_standalone_binary(args) {
    eprintln!(
      "{}: {}",
      deno_runtime::colors::red_bold("error"),
      err.to_string()
    );
    std::process::exit(1);
  }
}
