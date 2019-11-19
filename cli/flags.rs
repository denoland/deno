// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use log::Level;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenoFlags {
  pub log_level: Option<Level>,
  pub version: bool,
  pub reload: bool,
  pub config_path: Option<String>,
  pub import_map_path: Option<String>,
  pub allow_read: bool,
  pub read_whitelist: Vec<String>,
  pub cache_blacklist: Vec<String>,
  pub allow_write: bool,
  pub write_whitelist: Vec<String>,
  pub allow_net: bool,
  pub net_whitelist: Vec<String>,
  pub allow_env: bool,
  pub allow_run: bool,
  pub allow_hrtime: bool,
  pub no_prompts: bool,
  pub no_fetch: bool,
  pub seed: Option<u64>,
  pub v8_flags: Option<Vec<String>>,
  // Use tokio::runtime::current_thread
  pub current_thread: bool,

  pub lock: Option<String>,
  pub lock_write: bool,
}
