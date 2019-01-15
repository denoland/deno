// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::libdeno;

use getopts;
use getopts::Options;
use libc::c_int;
use std::ffi::CStr;
use std::ffi::CString;
use std::mem;
use std::vec::Vec;

// Creates vector of strings, Vec<String>
#[cfg(test)]
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenoFlags {
  pub help: bool,
  pub log_debug: bool,
  pub version: bool,
  pub reload: bool,
  pub recompile: bool,
  pub allow_write: bool,
  pub allow_net: bool,
  pub allow_env: bool,
  pub allow_run: bool,
  pub types: bool,
  pub prefetch: bool,
}

pub fn get_usage(opts: &Options) -> String {
  format!(
    "Usage: deno script.ts {}
Environment variables:
        DENO_DIR        Set deno's base directory.",
    opts.usage("")
  )
}

/// Checks provided arguments for known options and sets appropriate Deno flags
/// for them. Unknown options are returned for further use.
/// Note:
///
/// 1. This assumes that privileged flags do not accept parameters deno --foo bar.
/// This assumption is currently valid. But if it were to change in the future,
/// this parsing technique would need to be modified. I think we want to keep the
/// privileged flags minimal - so having this restriction is maybe a good thing.
///
/// 2. Misspelled flags will be forwarded to user code - e.g. --allow-ne would
/// not cause an error. I also think this is ok because missing any of the
/// privileged flags is not destructive. Userland flag parsing would catch these
/// errors.
fn set_recognized_flags(
  opts: &Options,
  flags: &mut DenoFlags,
  args: Vec<String>,
) -> Result<Vec<String>, getopts::Fail> {
  let mut rest = Vec::<String>::new();
  // getopts doesn't allow parsing unknown options so we check them
  // one-by-one and handle unrecognized ones manually
  // better solution welcome!
  for arg in args {
    let fake_args = vec![arg];
    match opts.parse(&fake_args) {
      Err(getopts::Fail::UnrecognizedOption(_)) => {
        rest.extend(fake_args);
      }
      Err(e) => {
        return Err(e);
      }
      Ok(matches) => {
        if matches.opt_present("help") {
          flags.help = true;
        }
        if matches.opt_present("log-debug") {
          flags.log_debug = true;
        }
        if matches.opt_present("version") {
          flags.version = true;
        }
        if matches.opt_present("reload") {
          flags.reload = true;
        }
        if matches.opt_present("recompile") {
          flags.recompile = true;
        }
        if matches.opt_present("allow-write") {
          flags.allow_write = true;
        }
        if matches.opt_present("allow-net") {
          flags.allow_net = true;
        }
        if matches.opt_present("allow-env") {
          flags.allow_env = true;
        }
        if matches.opt_present("allow-run") {
          flags.allow_run = true;
        }
        if matches.opt_present("allow-all") {
          flags.allow_env = true;
          flags.allow_net = true;
          flags.allow_run = true;
          flags.allow_write = true;
        }
        if matches.opt_present("types") {
          flags.types = true;
        }
        if matches.opt_present("prefetch") {
          flags.prefetch = true;
        }

        if !matches.free.is_empty() {
          rest.extend(matches.free);
        }
      }
    }
  }
  Ok(rest)
}

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub fn set_flags(
  args: Vec<String>,
) -> Result<(DenoFlags, Vec<String>, String), String> {
  // TODO: all flags passed after "--" are swallowed by v8_set_flags
  // eg. deno --allow-net ./test.ts -- --title foobar
  // args === ["deno", "--allow-net" "./test.ts"]
  let args = v8_set_flags(args);

  let mut opts = Options::new();
  // TODO(kevinkassimo): v8_set_flags intercepts '-help' with single '-'
  // Resolve that and then uncomment line below (enabling Go style -long-flag)
  // opts.long_only(true);
  opts.optflag("", "allow-write", "Allow file system write access.");
  opts.optflag("", "allow-net", "Allow network access.");
  opts.optflag("", "allow-env", "Allow environment access.");
  opts.optflag("", "allow-run", "Allow running subprocesses.");
  opts.optflag("A", "allow-all", "Allow all permissions");
  opts.optflag("", "recompile", "Force recompilation of TypeScript code.");
  opts.optflag("h", "help", "Print this message.");
  opts.optflag("D", "log-debug", "Log debug output.");
  opts.optflag("v", "version", "Print the version.");
  opts.optflag("r", "reload", "Reload cached remote resources.");
  opts.optflag("", "v8-options", "Print V8 command line options.");
  opts.optflag("", "types", "Print runtime TypeScript declarations.");
  opts.optflag("", "prefetch", "Prefetch the dependencies.");

  let mut flags = DenoFlags::default();

  let rest =
    set_recognized_flags(&opts, &mut flags, args).map_err(|e| e.to_string())?;
  Ok((flags, rest, get_usage(&opts)))
}

#[test]
fn test_set_flags_1() {
  let (flags, rest, _) = set_flags(svec!["deno", "--version"]).unwrap();
  assert_eq!(rest, svec!["deno"]);
  assert_eq!(
    flags,
    DenoFlags {
      version: true,
      ..DenoFlags::default()
    }
  );
}

#[test]
fn test_set_flags_2() {
  let (flags, rest, _) =
    set_flags(svec!["deno", "-r", "-D", "script.ts"]).unwrap();
  assert_eq!(rest, svec!["deno", "script.ts"]);
  assert_eq!(
    flags,
    DenoFlags {
      log_debug: true,
      reload: true,
      ..DenoFlags::default()
    }
  );
}

#[test]
fn test_set_flags_3() {
  let (flags, rest, _) =
    set_flags(svec!["deno", "-r", "script.ts", "--allow-write"]).unwrap();
  assert_eq!(rest, svec!["deno", "script.ts"]);
  assert_eq!(
    flags,
    DenoFlags {
      reload: true,
      allow_write: true,
      ..DenoFlags::default()
    }
  );
}

#[test]
fn test_set_flags_4() {
  let (flags, rest, _) =
    set_flags(svec!["deno", "-Dr", "script.ts", "--allow-write"]).unwrap();
  assert_eq!(rest, svec!["deno", "script.ts"]);
  assert_eq!(
    flags,
    DenoFlags {
      log_debug: true,
      reload: true,
      allow_write: true,
      ..DenoFlags::default()
    }
  );
}

#[test]
fn test_set_flags_5() {
  let (flags, rest, _) = set_flags(svec!["deno", "--types"]).unwrap();
  assert_eq!(rest, svec!["deno"]);
  assert_eq!(
    flags,
    DenoFlags {
      types: true,
      ..DenoFlags::default()
    }
  )
}

#[test]
fn test_set_flags_6() {
  let (flags, rest, _) =
    set_flags(svec!["deno", "gist.ts", "--title", "X", "--allow-net"]).unwrap();
  assert_eq!(rest, svec!["deno", "gist.ts", "--title", "X"]);
  assert_eq!(
    flags,
    DenoFlags {
      allow_net: true,
      ..DenoFlags::default()
    }
  )
}

#[test]
fn test_set_flags_7() {
  let (flags, rest, _) =
    set_flags(svec!["deno", "gist.ts", "--allow-all"]).unwrap();
  assert_eq!(rest, svec!["deno", "gist.ts"]);
  assert_eq!(
    flags,
    DenoFlags {
      allow_net: true,
      allow_env: true,
      allow_run: true,
      allow_write: true,
      ..DenoFlags::default()
    }
  )
}

// Returns args passed to V8, followed by args passed to JS
fn v8_set_flags_preprocess(args: Vec<String>) -> (Vec<String>, Vec<String>) {
  let (rest, mut v8_args) =
    args.into_iter().partition(|ref a| a.as_str() == "--help");

  // Replace args being sent to V8
  for a in &mut v8_args {
    if a == "--v8-options" {
      mem::swap(a, &mut String::from("--help"));
    }
  }
  (v8_args, rest)
}

#[test]
fn test_v8_set_flags_preprocess_1() {
  let js_args = v8_set_flags_preprocess(vec![
    "deno".to_string(),
    "--v8-options".to_string(),
  ]);
  assert_eq!(
    js_args,
    (vec!["deno".to_string(), "--help".to_string()], vec![])
  );
}

#[test]
fn test_v8_set_flags_preprocess_2() {
  let js_args =
    v8_set_flags_preprocess(vec!["deno".to_string(), "--help".to_string()]);
  assert_eq!(
    js_args,
    (vec!["deno".to_string()], vec!["--help".to_string()])
  );
}

// Pass the command line arguments to v8.
// Returns a vector of command line arguments that v8 did not understand.
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub fn v8_set_flags(args: Vec<String>) -> Vec<String> {
  // deno_set_v8_flags(int* argc, char** argv) mutates argc and argv to remove
  // flags that v8 understands.
  // First parse core args, then convert to a vector of C strings.
  let (args, rest) = v8_set_flags_preprocess(args);

  // Make a new array, that can be modified by V8::SetFlagsFromCommandLine(),
  // containing mutable raw pointers to the individual command line args.
  let mut raw_argv = args
    .iter()
    .map(|arg| CString::new(arg.as_str()).unwrap().into_bytes_with_nul())
    .collect::<Vec<_>>();
  let mut c_argv = raw_argv
    .iter_mut()
    .map(|arg| arg.as_mut_ptr() as *mut i8)
    .collect::<Vec<_>>();

  // Store the length of the c_argv array in a local variable. We'll pass
  // a pointer to this local variable to deno_set_v8_flags(), which then
  // updates its value.
  let mut c_argv_len = c_argv.len() as c_int;
  // Let v8 parse the arguments it recognizes and remove them from c_argv.
  unsafe {
    libdeno::deno_set_v8_flags(&mut c_argv_len, c_argv.as_mut_ptr());
  };
  // If c_argv_len was updated we have to change the length of c_argv to match.
  c_argv.truncate(c_argv_len as usize);
  // Copy the modified arguments list into a proper rust vec and return it.
  c_argv
    .iter()
    .map(|ptr| unsafe {
      let cstr = CStr::from_ptr(*ptr as *const i8);
      let slice = cstr.to_str().unwrap();
      slice.to_string()
    }).chain(rest.into_iter())
    .collect()
}
