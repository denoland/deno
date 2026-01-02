// Copyright 2018-2025 the Deno authors. MIT license.

// Copyright (c) The Rust Project Contributors

// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:

// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

// https://github.com/rust-lang/rust/blob/2eef47813f25df637026ce3288880e5c587abd92/library/std/src/sys/process/env.rs
use std::cmp;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::ffi::c_void;
use std::fmt;
use std::io;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::TRUE;
use windows_sys::Win32::Globalization::CSTR_EQUAL;
use windows_sys::Win32::Globalization::CSTR_GREATER_THAN;
use windows_sys::Win32::Globalization::CSTR_LESS_THAN;
use windows_sys::Win32::Globalization::CompareStringOrdinal;

/// Stores a set of changes to an environment
#[derive(Clone, Default)]
pub struct CommandEnv {
  clear: bool,
  saw_path: bool,
  vars: BTreeMap<EnvKey, Option<OsString>>,
}

impl fmt::Debug for CommandEnv {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut debug_command_env = f.debug_struct("CommandEnv");
    debug_command_env
      .field("clear", &self.clear)
      .field("vars", &self.vars);
    debug_command_env.finish()
  }
}

impl CommandEnv {
  // Capture the current environment with these changes applied
  pub fn capture(&self) -> BTreeMap<EnvKey, OsString> {
    let mut result = BTreeMap::<EnvKey, OsString>::new();
    if !self.clear {
      for (k, v) in env::vars_os() {
        result.insert(k.into(), v);
      }
    }
    for (k, maybe_v) in &self.vars {
      if let Some(v) = maybe_v {
        result.insert(k.clone(), v.clone());
      } else {
        result.remove(k);
      }
    }
    result
  }

  pub fn is_unchanged(&self) -> bool {
    !self.clear && self.vars.is_empty()
  }

  pub fn capture_if_changed(&self) -> Option<BTreeMap<EnvKey, OsString>> {
    if self.is_unchanged() {
      None
    } else {
      Some(self.capture())
    }
  }

  // The following functions build up changes
  pub fn set(&mut self, key: &OsStr, value: &OsStr) {
    let key = EnvKey::from(key);
    self.maybe_saw_path(&key);
    self.vars.insert(key, Some(value.to_owned()));
  }

  pub fn clear(&mut self) {
    self.clear = true;
    self.vars.clear();
  }

  pub fn have_changed_path(&self) -> bool {
    self.saw_path || self.clear
  }

  fn maybe_saw_path(&mut self, key: &EnvKey) {
    if !self.saw_path && key == "PATH" {
      self.saw_path = true;
    }
  }
}

// https://github.com/rust-lang/rust/blob/2eef47813f25df637026ce3288880e5c587abd92/library/std/src/sys/process/windows.rs
#[derive(Clone, Debug, Eq)]
#[doc(hidden)]
pub struct EnvKey {
  os_string: OsString,
  // This stores a UTF-16 encoded string to workaround the mismatch between
  // Rust's OsString (WTF-8) and the Windows API string type (UTF-16).
  // Normally converting on every API call is acceptable but here
  // `c::CompareStringOrdinal` will be called for every use of `==`.
  utf16: Vec<u16>,
}

impl EnvKey {
  pub fn new<T: Into<OsString>>(key: T) -> Self {
    EnvKey::from(key.into())
  }
}

// Comparing Windows environment variable keys[1] are behaviorally the
// composition of two operations[2]:
//
// 1. Case-fold both strings. This is done using a language-independent
// uppercase mapping that's unique to Windows (albeit based on data from an
// older Unicode spec). It only operates on individual UTF-16 code units so
// surrogates are left unchanged. This uppercase mapping can potentially change
// between Windows versions.
//
// 2. Perform an ordinal comparison of the strings. A comparison using ordinal
// is just a comparison based on the numerical value of each UTF-16 code unit[3].
//
// Because the case-folding mapping is unique to Windows and not guaranteed to
// be stable, we ask the OS to compare the strings for us. This is done by
// calling `CompareStringOrdinal`[4] with `bIgnoreCase` set to `TRUE`.
//
// [1] https://docs.microsoft.com/en-us/dotnet/standard/base-types/best-practices-strings#choosing-a-stringcomparison-member-for-your-method-call
// [2] https://docs.microsoft.com/en-us/dotnet/standard/base-types/best-practices-strings#stringtoupper-and-stringtolower
// [3] https://docs.microsoft.com/en-us/dotnet/api/system.stringcomparison?view=net-5.0#System_StringComparison_Ordinal
// [4] https://docs.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringordinal
impl Ord for EnvKey {
  fn cmp(&self, other: &Self) -> cmp::Ordering {
    unsafe {
      let result = CompareStringOrdinal(
        self.utf16.as_ptr(),
        self.utf16.len() as _,
        other.utf16.as_ptr(),
        other.utf16.len() as _,
        TRUE,
      );
      match result {
        CSTR_LESS_THAN => cmp::Ordering::Less,
        CSTR_EQUAL => cmp::Ordering::Equal,
        CSTR_GREATER_THAN => cmp::Ordering::Greater,
        // `CompareStringOrdinal` should never fail so long as the parameters are correct.
        _ => panic!(
          "comparing environment keys failed: {}",
          std::io::Error::last_os_error()
        ),
      }
    }
  }
}
impl PartialOrd for EnvKey {
  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
    Some(self.cmp(other))
  }
}
impl PartialEq for EnvKey {
  fn eq(&self, other: &Self) -> bool {
    if self.utf16.len() != other.utf16.len() {
      false
    } else {
      self.cmp(other) == cmp::Ordering::Equal
    }
  }
}
impl PartialOrd<str> for EnvKey {
  fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
    Some(self.cmp(&EnvKey::new(other)))
  }
}
impl PartialEq<str> for EnvKey {
  fn eq(&self, other: &str) -> bool {
    if self.os_string.len() != other.len() {
      false
    } else {
      self.cmp(&EnvKey::new(other)) == cmp::Ordering::Equal
    }
  }
}

// Environment variable keys should preserve their original case even though
// they are compared using a caseless string mapping.
impl From<OsString> for EnvKey {
  fn from(k: OsString) -> Self {
    EnvKey {
      utf16: k.encode_wide().collect(),
      os_string: k,
    }
  }
}

impl From<&OsStr> for EnvKey {
  fn from(k: &OsStr) -> Self {
    Self::from(k.to_os_string())
  }
}

pub fn ensure_no_nuls<T: AsRef<OsStr>>(s: T) -> io::Result<T> {
  if s.as_ref().encode_wide().any(|b| b == 0) {
    Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "nul byte found in provided data",
    ))
  } else {
    Ok(s)
  }
}

pub fn make_envp(
  maybe_env: Option<BTreeMap<EnvKey, OsString>>,
) -> io::Result<(*mut c_void, Vec<u16>)> {
  // On Windows we pass an "environment block" which is not a char**, but
  // rather a concatenation of null-terminated k=v\0 sequences, with a final
  // \0 to terminate.
  if let Some(env) = maybe_env {
    let mut blk = Vec::new();

    // If there are no environment variables to set then signal this by
    // pushing a null.
    if env.is_empty() {
      blk.push(0);
    }

    for (k, v) in env {
      ensure_no_nuls(k.os_string)?;
      blk.extend(k.utf16);
      blk.push('=' as u16);
      blk.extend(ensure_no_nuls(v)?.encode_wide());
      blk.push(0);
    }
    blk.push(0);
    Ok((blk.as_mut_ptr() as *mut c_void, blk))
  } else {
    Ok((std::ptr::null_mut(), Vec::new()))
  }
}
