use deno_core::error::custom_error;
use deno_core::error::AnyError;

/// A utility function to map OsStrings to Strings
pub fn into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {:?} is not valid UTF-8", s);
    custom_error("InvalidData", message)
  })
}
