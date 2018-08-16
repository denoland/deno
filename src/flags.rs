// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding;
use libc::c_int;
use std::ffi::CStr;
use std::ffi::CString;
use std::mem;

// Creates vector of strings, Vec<String>
#[cfg(test)]
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[derive(Debug, PartialEq)]
pub struct DenoFlags {
  pub help: bool,
  pub log_debug: bool,
  pub version: bool,
  pub reload: bool,
  pub allow_write: bool,
  pub allow_net: bool,
}

pub fn print_usage() {
  println!(
    "Usage: deno script.js

--allow-write      Allow file system write access.
--allow-net        Allow network access.
-v or --version    Print the version.
-r or --reload     Reload cached remote resources.
-D or --log-debug  Log debug output.
--help             Print this message.
--v8-options       Print V8 command line options."
  );
}

// Parses flags for deno. This does not do v8_set_flags() - call that separately.
pub fn set_flags(args: Vec<String>) -> (DenoFlags, Vec<String>) {
  let mut flags = DenoFlags {
    help: false,
    version: false,
    reload: false,
    log_debug: false,
    allow_write: false,
    allow_net: false,
  };
  let mut rest = Vec::new();
  for a in &args {
    match a.as_str() {
      "-h" | "--help" => flags.help = true,
      "-D" | "--log-debug" => flags.log_debug = true,
      "-v" | "--version" => flags.version = true,
      "-r" | "--reload" => flags.reload = true,
      "--allow-write" => flags.allow_write = true,
      "--allow-net" => flags.allow_net = true,
      _ => rest.push(a.clone()),
    }
  }

  return (flags, rest);
}

#[test]
fn test_set_flags_1() {
  let (flags, rest) = set_flags(svec!["deno", "--version"]);
  assert!(rest == svec!["deno"]);
  assert!(
    flags == DenoFlags {
      help: false,
      log_debug: false,
      version: true,
      reload: false,
      allow_write: false,
      allow_net: false,
    }
  );
}

#[test]
fn test_set_flags_2() {
  let (flags, rest) = set_flags(svec!["deno", "-r", "-D", "script.ts"]);
  assert!(rest == svec!["deno", "script.ts"]);
  assert!(
    flags == DenoFlags {
      help: false,
      log_debug: true,
      version: false,
      reload: true,
      allow_write: false,
      allow_net: false,
    }
  );
}

#[test]
fn test_set_flags_3() {
  let (flags, rest) =
    set_flags(svec!["deno", "-r", "script.ts", "--allow-write"]);
  assert!(rest == svec!["deno", "script.ts"]);
  assert!(
    flags == DenoFlags {
      help: false,
      log_debug: false,
      version: false,
      reload: true,
      allow_write: true,
      allow_net: false,
    }
  );
}

// Returns args passed to V8, followed by args passed to JS
// TODO Rename to v8_set_flags_preprocess
fn parse_core_args(args: Vec<String>) -> (Vec<String>, Vec<String>) {
  let mut rest = vec![];

  // Filter out args that shouldn't be passed to V8
  let mut args: Vec<String> = args
    .into_iter()
    .filter(|arg| {
      if arg.as_str() == "--help" {
        rest.push(arg.clone());
        return false;
      }

      true
    })
    .collect();

  // Replace args being sent to V8
  for idx in 0..args.len() {
    if args[idx] == "--v8-options" {
      mem::swap(args.get_mut(idx).unwrap(), &mut String::from("--help"));
    }
  }

  (args, rest)
}

#[test]
fn test_parse_core_args_1() {
  let js_args =
    parse_core_args(vec!["deno".to_string(), "--v8-options".to_string()]);
  assert!(js_args == (vec!["deno".to_string(), "--help".to_string()], vec![]));
}

#[test]
fn test_parse_core_args_2() {
  let js_args = parse_core_args(vec!["deno".to_string(), "--help".to_string()]);
  assert!(js_args == (vec!["deno".to_string()], vec!["--help".to_string()]));
}

// Pass the command line arguments to v8.
// Returns a vector of command line arguments that v8 did not understand.
pub fn v8_set_flags(args: Vec<String>) -> Vec<String> {
  // deno_set_flags(int* argc, char** argv) mutates argc and argv to remove
  // flags that v8 understands.
  // First parse core args, then converto to a vector of C strings.
  let (argv, rest) = parse_core_args(args);
  let mut argv = argv
    .iter()
    .map(|arg| CString::new(arg.as_str()).unwrap().into_bytes_with_nul())
    .collect::<Vec<_>>();

  // Make a new array, that can be modified by V8::SetFlagsFromCommandLine(),
  // containing mutable raw pointers to the individual command line args.
  let mut c_argv = argv
    .iter_mut()
    .map(|arg| arg.as_mut_ptr() as *mut i8)
    .collect::<Vec<_>>();
  // Store the length of the argv array in a local variable. We'll pass a
  // pointer to this local variable to deno_set_flags(), which then
  // updates its value.
  let mut c_argc = c_argv.len() as c_int;
  // Let v8 parse the arguments it recognizes and remove them from c_argv.
  unsafe {
    // TODO(ry) Rename deno_set_flags to deno_set_v8_flags().
    binding::deno_set_flags(&mut c_argc, c_argv.as_mut_ptr());
  };
  // If c_argc was updated we have to change the length of c_argv to match.
  c_argv.truncate(c_argc as usize);
  // Copy the modified arguments list into a proper rust vec and return it.
  c_argv
    .iter()
    .map(|ptr| unsafe {
      let cstr = CStr::from_ptr(*ptr as *const i8);
      let slice = cstr.to_str().unwrap();
      slice.to_string()
    })
    .chain(rest.into_iter())
    .collect()
}
