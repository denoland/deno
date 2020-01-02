// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//! This module wraps libdeno::deno_set_v8_flags
use crate::libdeno::deno_set_v8_flags;
use libc::c_char;
use libc::c_int;
use std::ffi::CStr;
use std::ffi::CString;
use std::vec::Vec;

/// Pass the command line arguments to v8.
/// Returns a vector of command line arguments that V8 did not understand.
pub fn v8_set_flags(args: Vec<String>) -> Vec<String> {
  // deno_set_v8_flags(int* argc, char** argv) mutates argc and argv to remove
  // flags that v8 understands.

  // Make a new array, that can be modified by V8::SetFlagsFromCommandLine(),
  // containing mutable raw pointers to the individual command line args.
  let mut raw_argv = args
    .iter()
    .map(|arg| CString::new(arg.as_str()).unwrap().into_bytes_with_nul())
    .collect::<Vec<_>>();
  let mut c_argv = raw_argv
    .iter_mut()
    .map(|arg| arg.as_mut_ptr() as *mut c_char)
    .collect::<Vec<_>>();

  // Store the length of the c_argv array in a local variable. We'll pass
  // a pointer to this local variable to deno_set_v8_flags(), which then
  // updates its value.
  let mut c_argv_len = c_argv.len() as c_int;
  // Let v8 parse the arguments it recognizes and remove them from c_argv.
  unsafe { deno_set_v8_flags(&mut c_argv_len, c_argv.as_mut_ptr()) };
  // If c_argv_len was updated we have to change the length of c_argv to match.
  c_argv.truncate(c_argv_len as usize);
  // Copy the modified arguments list into a proper rust vec and return it.
  c_argv
    .iter()
    .map(|ptr| unsafe {
      let cstr = CStr::from_ptr(*ptr as *const c_char);
      let slice = cstr.to_str().unwrap();
      slice.to_string()
    })
    .collect()
}
