// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::env;

type Snapshot = BTreeMap<String, String>;

pub fn make_snapshot() -> Snapshot {
  return env::vars().collect();
}

pub fn restore_snapshot(old: &Snapshot) {
  env::vars()
    .filter(|(k, _)| !old.contains_key(k))
    .for_each(|(k, _)| env::remove_var(k));
  old.iter().for_each(|(k, v)| env::set_var(k, v));
}

pub fn reset_env_func() -> impl Fn() {
  let current_env = make_snapshot();
  return move || restore_snapshot(&current_env);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_snapshots() {
    env::set_var("DENO_HELLO_WORLD", "Hello World!");
    env::set_var("DENO_FOOBAR", "foobar");
    let reset_env = reset_env_func();

    env::remove_var("DENO_HELLO_WORLD");
    env::set_var("DENO_RUSTY_SPOONS", "there is no spoon");

    reset_env();
    assert!(env::var("DENO_HELLO_WORLD").unwrap() == "Hello World!");
    assert!(env::var("DENO_FOOBAR").unwrap() == "foobar");
    assert!(env::var("DENO_RUSTY_SPOONS").is_err());
  }
}
