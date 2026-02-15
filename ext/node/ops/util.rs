// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::OpState;
use deno_core::ResourceHandle;
use deno_core::ResourceHandleFd;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::ExtNodeSys;
use crate::NodeResolverRc;

#[repr(u32)]
enum HandleType {
  #[allow(dead_code)]
  Tcp = 0,
  Tty,
  #[allow(dead_code)]
  Udp,
  File,
  Pipe,
  Unknown,
}

/// Check if a raw file descriptor is a TTY.
/// This is used by Node.js `tty.isatty(fd)`.
#[op2(fast)]
pub fn op_node_is_tty(fd: i32) -> bool {
  if fd < 0 {
    return false;
  }
  is_tty(fd)
}

#[cfg(unix)]
fn is_tty(fd: i32) -> bool {
  // SAFETY: We're checking if the fd is a terminal.
  // The fd may or may not be valid, but libc::isatty handles that safely.
  unsafe { libc::isatty(fd) == 1 }
}

#[cfg(windows)]
fn is_tty(fd: i32) -> bool {
  use winapi::um::consoleapi::GetConsoleMode;
  use winapi::um::processenv::GetStdHandle;
  use winapi::um::winbase::STD_ERROR_HANDLE;
  use winapi::um::winbase::STD_INPUT_HANDLE;
  use winapi::um::winbase::STD_OUTPUT_HANDLE;

  // SAFETY: GetStdHandle returns a borrowed handle to stdin/stdout/stderr.
  // For fd > 2, we try to use it as a raw handle directly.
  let handle = match fd {
    // SAFETY: These are valid standard handles.
    0 => unsafe { GetStdHandle(STD_INPUT_HANDLE) },
    // SAFETY: These are valid standard handles.
    1 => unsafe { GetStdHandle(STD_OUTPUT_HANDLE) },
    // SAFETY: These are valid standard handles.
    2 => unsafe { GetStdHandle(STD_ERROR_HANDLE) },
    _ => fd as winapi::um::winnt::HANDLE,
  };

  let mut mode = 0;
  // SAFETY: handle is either a valid standard handle or a raw fd cast to HANDLE.
  // GetConsoleMode will return 0 if the handle is invalid or not a console.
  unsafe { GetConsoleMode(handle, &mut mode) != 0 }
}

#[op2(fast)]
pub fn op_node_guess_handle_type(state: &mut OpState, rid: u32) -> u32 {
  let handle = match state.resource_table.get_handle(rid) {
    Ok(handle) => handle,
    _ => return HandleType::Unknown as u32,
  };

  let handle_type = match handle {
    ResourceHandle::Fd(handle) => guess_handle_type(handle),
    _ => HandleType::Unknown,
  };

  handle_type as u32
}

#[cfg(windows)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use winapi::um::consoleapi::GetConsoleMode;
  use winapi::um::fileapi::GetFileType;
  use winapi::um::winbase::FILE_TYPE_CHAR;
  use winapi::um::winbase::FILE_TYPE_DISK;
  use winapi::um::winbase::FILE_TYPE_PIPE;

  // SAFETY: Call to win32 fileapi. `handle` is a valid fd.
  match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK => HandleType::File,
    FILE_TYPE_CHAR => {
      let mut mode = 0;
      // SAFETY: Call to win32 consoleapi. `handle` is a valid fd.
      //         `mode` is a valid pointer.
      if unsafe { GetConsoleMode(handle, &mut mode) } == 1 {
        HandleType::Tty
      } else {
        HandleType::File
      }
    }
    FILE_TYPE_PIPE => HandleType::Pipe,
    _ => HandleType::Unknown,
  }
}

#[cfg(unix)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use std::io::IsTerminal;
  // SAFETY: The resource remains open for the duration of borrow_raw.
  if unsafe { std::os::fd::BorrowedFd::borrow_raw(handle).is_terminal() } {
    return HandleType::Tty;
  }

  // SAFETY: It is safe to zero-initialize a `libc::stat` struct.
  let mut s = unsafe { std::mem::zeroed() };
  // SAFETY: Call to libc
  if unsafe { libc::fstat(handle, &mut s) } == 1 {
    return HandleType::Unknown;
  }

  match s.st_mode & 61440 {
    libc::S_IFREG | libc::S_IFCHR => HandleType::File,
    libc::S_IFIFO => HandleType::Pipe,
    libc::S_IFSOCK => HandleType::Tcp,
    _ => HandleType::Unknown,
  }
}

#[op2(fast)]
pub fn op_node_view_has_buffer(buffer: v8::Local<v8::ArrayBufferView>) -> bool {
  buffer.has_buffer()
}

/// Checks if the current call site is from a dependency package.
#[op2(fast)]
pub fn op_node_call_is_from_dependency<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
) -> bool {
  // non internal call site should appear in < 20 frames
  let Some(stack_trace) = v8::StackTrace::current_stack_trace(scope, 20) else {
    return false;
  };
  let mut only_internal = true;
  for i in 0..stack_trace.get_frame_count() {
    let Some(frame) = stack_trace.get_frame(scope, i) else {
      continue;
    };
    if !frame.is_user_javascript() {
      continue;
    }
    let Some(script) = frame.get_script_name(scope) else {
      continue;
    };
    let name = script.to_rust_string_lossy(scope);

    if name.starts_with("node:") || name.starts_with("ext:") {
      continue;
    } else {
      only_internal = false;
    }

    if name.starts_with("https:")
      || name.contains("/node_modules/")
      || name.contains(r"\node_modules\")
    {
      return true;
    }

    let Ok(specifier) = url::Url::parse(&name) else {
      continue;
    };
    if only_internal {
      return true;
    }
    return state.borrow::<NodeResolverRc<
        TInNpmPackageChecker,
        TNpmPackageFolderResolver,
        TSys,
      >>().in_npm_package(&specifier);
  }
  only_internal
}

#[op2(fast)]
pub fn op_node_in_npm_package<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] path: &str,
) -> bool {
  let specifier = if deno_path_util::specifier_has_uri_scheme(path) {
    match url::Url::parse(path) {
      Ok(url) => url,
      Err(_) => return false,
    }
  } else {
    match deno_path_util::url_from_file_path(Path::new(path)) {
      Ok(url) => url,
      Err(_) => return false,
    }
  };

  state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>().in_npm_package(&specifier)
}

#[op2]
pub fn op_node_get_own_non_index_properties<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  #[smi] filter: u32,
) -> Result<v8::Local<'s, v8::Array>, JsErrorBox> {
  let mut property_filter = v8::PropertyFilter::ALL_PROPERTIES;
  if filter & 1 << 0 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_WRITABLE;
  }
  if filter & 1 << 1 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_ENUMERABLE;
  }
  if filter & 1 << 2 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_CONFIGURABLE;
  }
  if filter & 1 << 3 != 0 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_STRINGS;
  }
  if filter & 1 << 4 != 0 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_SYMBOLS;
  }

  obj
    .get_property_names(
      scope,
      v8::GetPropertyNamesArgs {
        index_filter: v8::IndexFilter::SkipIndices,
        property_filter,
        key_conversion: v8::KeyConversionMode::NoNumbers,
        mode: v8::KeyCollectionMode::OwnOnly,
      },
    )
    .ok_or_else(|| {
      JsErrorBox::type_error("Failed to get own non-index properties")
    })
}

// Removes leading and trailing spaces from a string.
// Returns an empty string if the input is empty/whitespace.
// Example:
//   trim_spaces("  hello  ") -> "hello"
//   trim_spaces("") -> ""
fn trim_spaces(input: &str) -> &str {
  input.trim_matches(|c| matches!(c, ' ' | '\t' | '\n'))
}

#[op2]
pub fn op_node_parse_env<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[string] input: &str,
) -> v8::Local<'a, v8::Object> {
  let vars = v8::Object::new(scope);

  // Ported from https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/src/node_dotenv.cc#L127-L303
  {
    // Handle windows newlines "\r\n": remove "\r" and keep only "\n"
    let lines = input.replace('\r', "");
    let mut content = trim_spaces(&lines);

    while !content.is_empty() {
      // Skip empty lines and comments
      if content.starts_with('\n') || content.starts_with('#') {
        // Check if the first character of the content is a newline or a hash
        if let Some(newline) = content.find('\n') {
          // Remove everything up to and including the newline character
          content = &content[newline + 1..];
        } else {
          // If no newline is found, clear the content
          content = "";
        }

        // Skip the remaining code in the loop and continue with the next
        // iteration.
        continue;
      }

      // Find the next equals sign or newline in a single pass.
      // This optimizes the search by avoiding multiple iterations.
      let equal_or_newline = content.find(['=', '\n']);

      let Some(equal_or_newline) = equal_or_newline else {
        break;
      };

      // If we found a newline before equals, the line is invalid.
      if content.as_bytes()[equal_or_newline] == b'\n' {
        content = &content[equal_or_newline + 1..];
        content = trim_spaces(content);
        continue;
      }

      // We found an equals sign, extract the key
      let mut key = trim_spaces(&content[..equal_or_newline]);
      content = &content[equal_or_newline + 1..];

      // If the value is not present (e.g. KEY=) set it to an empty string
      if content.is_empty() || content.starts_with('\n') {
        let key_v8 = v8::String::new(scope, key).unwrap();
        let value_v8 = v8::String::new(scope, "").unwrap();
        vars.set(scope, key_v8.into(), value_v8.into());
        continue;
      }

      content = trim_spaces(content);

      // Skip lines with empty keys after trimming spaces.
      // Examples of invalid keys that would be skipped:
      //   =value
      //   "   "=value
      if key.is_empty() {
        continue;
      }

      // Remove export prefix from key and ensure proper spacing.
      // Example: export FOO=bar -> FOO=bar
      if let Some(stripped) = key.strip_prefix("export ") {
        // Trim spaces after removing export prefix to handle cases like:
        // export   FOO=bar
        key = trim_spaces(stripped);
      }

      // In case the last line is a single key without value.
      // Example: KEY= (without a newline at the EOF)
      if content.is_empty() {
        let key_v8 = v8::String::new(scope, key).unwrap();
        let value_v8 = v8::String::new(scope, "").unwrap();
        vars.set(scope, key_v8.into(), value_v8.into());
        break;
      }

      let value: String;

      // Expand new line if \n is inside double quotes.
      // Example: EXPAND_NEWLINES="expand\nnew\nlines"
      if content.starts_with('"')
        && let Some(closing_quote) =
          content[1..].find('"').map(|index| index + 1)
      {
        // Replace \n with actual newlines in double-quoted strings
        value = content[1..closing_quote].replace("\\n", "\n");

        if let Some(newline) = content[closing_quote + 1..]
          .find('\n')
          .map(|index| index + closing_quote + 1)
        {
          content = &content[newline + 1..];
        } else {
          content = "";
        }

        let key_v8 = v8::String::new(scope, key).unwrap();
        let value_v8 = v8::String::new(scope, &value).unwrap();
        vars.set(scope, key_v8.into(), value_v8.into());
        continue;
      }

      if content.starts_with('"')
        || content.starts_with('\'')
        || content.starts_with('`')
      {
        let quote = content.as_bytes()[0];
        if let Some(closing_quote) =
          content[1..].find(quote as char).map(|index| index + 1)
        {
          // Found closing quote - take content between quotes
          value = content[1..closing_quote].to_string();

          if let Some(newline) = content[closing_quote + 1..]
            .find('\n')
            .map(|index| index + closing_quote + 1)
          {
            content = &content[newline + 1..];
          } else {
            content = "";
          }

          let key_v8 = v8::String::new(scope, key).unwrap();
          let value_v8 = v8::String::new(scope, &value).unwrap();
          vars.set(scope, key_v8.into(), value_v8.into());

          // No valid data here, skip to next line
          continue;
        }

        // Check if newline exists. If it does, take the entire line as the value.
        // Example: KEY="value\nKEY2=value2
        // The value pair should be `"value`
        if let Some(newline) = content.find('\n') {
          value = content[..newline].to_string();
          content = &content[newline + 1..];
        } else {
          // No newline - take rest of content
          value = content.to_string();
          content = "";
        }
      } else if let Some(newline) = content.find('\n') {
        // Regular key value pair.
        // Example: KEY=this is value
        let mut raw_value = &content[..newline];
        // Check if there is a comment in the line
        // Example: KEY=value # comment
        // The value pair should be `value`
        if let Some(hash_character) = raw_value.find('#') {
          raw_value = &raw_value[..hash_character];
        }
        value = trim_spaces(raw_value).to_string();
        content = &content[newline + 1..];
      } else {
        // Last line without newline
        let mut raw_value = content;
        if let Some(hash_character) = raw_value.find('#') {
          raw_value = &content[..hash_character];
        }
        value = trim_spaces(raw_value).to_string();
        content = "";
      }

      let key_v8 = v8::String::new(scope, key).unwrap();
      let value_v8 = v8::String::new(scope, &value).unwrap();
      vars.set(scope, key_v8.into(), value_v8.into());

      content = trim_spaces(content);
    }
  }

  vars
}
