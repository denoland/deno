// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// Pass the command line arguments to v8.
/// Returns a vector of command line arguments that V8 did not understand.
pub fn v8_set_flags(mut args: Vec<String>) -> Vec<String> {
  args.push("--no_freeze_flags_after_init".to_string());
  v8::V8::set_flags_from_command_line(args)
}
