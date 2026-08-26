// Copyright 2018-2026 the Deno authors. MIT license.

//! Replacements for the std `print!`, `println!`, `eprint!` and `eprintln!`
//! macros that drop write errors instead of panicking, named after Cargo's
//! macros with the same semantics.
//!
//! The Rust runtime ignores `SIGPIPE`, so when the other end of a pipe is
//! closed (ex. `deno test | head`) writes to stdout surface as `EPIPE`
//! errors, which make the std print macros panic. The macros in this crate
//! drop write errors instead, matching how Node.js treats `EPIPE` on stdout
//! and how `env_logger` treats write errors.
//!
//! ```
//! use deno_print::drop_println;
//!
//! drop_println!("hello there");
//! ```
//!
//! Use these macros for output that is the program's deliverable and must
//! go to stdout (ex. subcommand reporters). For diagnostic output prefer
//! the `log` crate, which is also tolerant to write errors and integrates
//! with the progress bar and telemetry.
//!
//! See https://github.com/denoland/deno/issues/15767

/// Writes pre-encoded bytes to stdout, dropping write errors instead of
/// surfacing them. The byte-slice sibling of [`drop_print!`] for output
/// that is not produced via format args (ex. serialized JSON, bundle
/// contents).
pub fn drop_write_stdout(bytes: &[u8]) {
  use std::io::Write as _;
  let _ = std::io::stdout().write_all(bytes);
}

/// Like `std::print!`, but drops write errors instead of panicking.
#[macro_export]
macro_rules! drop_print {
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::write!(::std::io::stdout(), $($arg)+);
  }};
}

/// Like `std::println!`, but drops write errors instead of panicking.
#[macro_export]
macro_rules! drop_println {
  () => {{
    use ::std::io::Write as _;
    let _ = ::std::writeln!(::std::io::stdout());
  }};
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::writeln!(::std::io::stdout(), $($arg)+);
  }};
}

/// Like `std::eprint!`, but drops write errors instead of panicking.
#[macro_export]
macro_rules! drop_eprint {
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::write!(::std::io::stderr(), $($arg)+);
  }};
}

/// Like `std::eprintln!`, but drops write errors instead of panicking.
#[macro_export]
macro_rules! drop_eprintln {
  () => {{
    use ::std::io::Write as _;
    let _ = ::std::writeln!(::std::io::stderr());
  }};
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::writeln!(::std::io::stderr(), $($arg)+);
  }};
}

#[cfg(test)]
mod tests {
  #[test]
  fn all_macro_forms_expand() {
    let value = 42;
    drop_print!("positional {} and named {value}", "arg");
    drop_println!();
    drop_println!("positional {} and named {value}", "arg");
    drop_println!("trailing comma {},", value,);
    drop_eprint!("positional {} and named {value}", "arg");
    drop_eprintln!();
    drop_eprintln!("positional {} and named {value}", "arg");
  }

  #[test]
  #[allow(
    clippy::disallowed_methods,
    reason = "test spawns itself as a child process"
  )]
  fn does_not_panic_on_closed_stdout() {
    if std::env::var_os("DENO_PRINT_TEST_CHILD").is_some() {
      // more than any pipe buffer holds, so writes keep happening after
      // the parent closes the read end
      for i in 0..1_000_000 {
        drop_println!("line {}", i);
      }
      // exit before the libtest harness prints its summary with the std
      // macros, which would panic on the closed stdout
      std::process::exit(0);
    }

    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
      .arg("tests::does_not_panic_on_closed_stdout")
      .arg("--exact")
      .env("DENO_PRINT_TEST_CHILD", "1")
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::null())
      .spawn()
      .unwrap();
    {
      use std::io::Read as _;
      let mut stdout = child.stdout.take().unwrap();
      let mut buf = [0u8; 100];
      let _ = stdout.read_exact(&mut buf);
      // dropping the read end makes the child's subsequent writes fail
    }
    let status = child.wait().unwrap();
    assert!(status.success(), "child exited with {:?}", status);
  }
}
