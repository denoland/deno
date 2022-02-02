// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// Pass the command line arguments to v8.
/// Returns a vector of command line arguments that V8 did not understand.
pub fn v8_set_flags(args: Vec<String>) -> Vec<String> {
  v8::V8::set_flags_from_command_line(args)
}
