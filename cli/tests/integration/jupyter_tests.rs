// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

itest!(jupyter_install_command_not_exists {
  args: "jupyter --unstable --install",
  output: "jupyter/install_command_not_exists.out",
  envs: vec![("PATH".to_string(), "".to_string())],
  exit_code: 1,
});
