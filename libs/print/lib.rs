// Copyright 2018-2026 the Deno authors. MIT license.

//! Drop-in replacements for the std `print!`, `println!`, `eprint!` and
//! `eprintln!` macros that ignore write errors instead of panicking.
//!
//! The Rust runtime ignores `SIGPIPE`, so when the other end of a pipe is
//! closed (ex. `deno test | head`) writes to stdout surface as `EPIPE`
//! errors, which make the std print macros panic. The macros in this crate
//! swallow write errors instead, matching how Node.js treats `EPIPE` on
//! stdout and how `env_logger` treats write errors.
//!
//! Import the macros to shadow the std prelude ones:
//!
//! ```
//! use deno_print::println;
//!
//! println!("hello there");
//! ```
//!
//! Use these macros for output that is the program's deliverable and must
//! go to stdout (ex. subcommand reporters). For diagnostic output prefer
//! the `log` crate, which is also tolerant to write errors and integrates
//! with the progress bar and telemetry.
//!
//! See https://github.com/denoland/deno/issues/15767

/// Like `std::print!`, but ignores write errors instead of panicking.
#[macro_export]
macro_rules! print {
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::write!(::std::io::stdout(), $($arg)+);
  }};
}

/// Like `std::println!`, but ignores write errors instead of panicking.
#[macro_export]
macro_rules! println {
  () => {{
    use ::std::io::Write as _;
    let _ = ::std::writeln!(::std::io::stdout());
  }};
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::writeln!(::std::io::stdout(), $($arg)+);
  }};
}

/// Like `std::eprint!`, but ignores write errors instead of panicking.
#[macro_export]
macro_rules! eprint {
  ($($arg:tt)+) => {{
    use ::std::io::Write as _;
    let _ = ::std::write!(::std::io::stderr(), $($arg)+);
  }};
}

/// Like `std::eprintln!`, but ignores write errors instead of panicking.
#[macro_export]
macro_rules! eprintln {
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
  // shadow the std prelude macros like consumers of this crate would
  use crate::eprint;
  use crate::eprintln;
  use crate::print;
  use crate::println;

  #[test]
  fn all_macro_forms_expand() {
    let value = 42;
    print!("positional {} and named {value}", "arg");
    println!();
    println!("positional {} and named {value}", "arg");
    println!("trailing comma {},", value,);
    eprint!("positional {} and named {value}", "arg");
    eprintln!();
    eprintln!("positional {} and named {value}", "arg");
  }

  #[test]
  fn does_not_panic_on_closed_stdout() {
    if std::env::var_os("DENO_PRINT_TEST_CHILD").is_some() {
      // more than any pipe buffer holds, so writes keep happening after
      // the parent closes the read end
      for i in 0..1_000_000 {
        println!("line {}", i);
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
