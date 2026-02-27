// Copyright 2018-2025 the Deno authors. MIT license.

/// Pass the command line arguments to v8.
/// The first element of args (which usually corresponds to the binary name) is
/// ignored.
/// Returns a vector of command line arguments that V8 did not understand.
pub fn v8_set_flags(args: Vec<String>) -> Vec<String> {
  v8::V8::set_flags_from_command_line(args)
}
