use crate::flags::{resolve_urls, Flags};
use deno_core::serde_json::Value;
use log::Level;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
enum StringBool {
  String(String),
  Bool(bool),
}

#[derive(Deserialize, Debug)]
pub struct Config {
  unstable: Option<bool>,
  log_level: Option<String>,
  pub quiet: Option<bool>,

  runtime: Option<Runtime>,
  pub test: Option<Test>,
  pub fmt: Option<Fmt>,
  pub lint: Option<Lint>,

  #[serde(flatten)]
  extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
struct Runtime {
  permissions: Option<Permissions>,
  v8_flags: Option<Vec<String>>,
  seed: Option<u64>,
  inspect: Option<StringBool>,
  inspect_brk: Option<StringBool>,

  cached_only: Option<bool>,
  import_map: Option<String>,
  no_remote: Option<bool>,
  config: Option<String>,
  no_check: Option<bool>,
  reload: Option<Vec<String>>,
  /*lock: Option<bool>,
  lock_write: Option<bool>,*/
  cert: Option<String>,

  #[serde(flatten)]
  extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
struct Permissions {
  read: Option<Vec<String>>,
  write: Option<Vec<String>>,
  net: Option<Vec<String>>,
  plugin: Option<bool>,
  run: Option<bool>,
  env: Option<bool>,
  hrtime: Option<bool>,
  all: Option<bool>,

  #[serde(flatten)]
  extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
pub struct Test {
  pub no_run: Option<bool>,
  pub fail_fast: Option<bool>,
  pub allow_none: Option<bool>,
  pub filter: Option<String>,
  pub coverage: Option<bool>,
  pub files: Option<Vec<String>>,

  #[serde(flatten)]
  extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
pub struct Fmt {
  pub check: Option<bool>,
  pub ignore: Option<Vec<String>>,
  pub files: Option<Vec<String>>,
  pub watch: Option<bool>,

  #[serde(flatten)]
  extra: HashMap<String, Value>,
}

#[derive(Deserialize, Debug)]
pub struct Lint {
  pub ignore: Option<Vec<String>>,
  pub files: Option<Vec<String>>,
  pub json: Option<bool>,

  #[serde(flatten)]
  extra: HashMap<String, Value>,
}

impl Config {
  pub fn to_flags(&self) -> Result<Flags, ()> {
    let mut flags = Flags::default();

    if !self.extra.is_empty() {
      return Err(());
    }

    flags.unstable = self.unstable.unwrap_or_default();

    if let Some(ref log_level) = self.log_level {
      flags.log_level = match log_level.as_str() {
        "debug" => Some(Level::Debug),
        "info" => Some(Level::Info),
        _ => return Err(()),
      }
    }
    if let Some(quiet) = self.quiet {
      if quiet {
        flags.log_level = Some(Level::Error);
      }
    }

    if let Some(ref runtime) = self.runtime {
      if !runtime.extra.is_empty() {
        return Err(());
      }

      if let Some(ref v8_flags) = runtime.v8_flags {
        flags.v8_flags = v8_flags.clone();
      }

      if let Some(seed) = runtime.seed {
        flags.seed = Some(seed);
        flags.v8_flags.push(format!("--random-seed={}", seed));
      }

      let default = || "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
      if let Some(ref inspect) = runtime.inspect {
        flags.inspect = match inspect {
          StringBool::String(host) => Some(host.parse().unwrap()),
          StringBool::Bool(true) => Some(default()),
          StringBool::Bool(false) => None,
        }
      }
      if let Some(ref inspect) = runtime.inspect_brk {
        flags.inspect_brk = match inspect {
          StringBool::String(host) => Some(host.parse().unwrap()),
          StringBool::Bool(true) => Some(default()),
          StringBool::Bool(false) => None,
        }
      }

      flags.cached_only = runtime.cached_only.unwrap_or_default();
      flags.import_map_path = runtime.import_map.clone();
      flags.no_remote = runtime.no_remote.unwrap_or_default();
      flags.config_path = runtime.config.clone();
      flags.no_check = runtime.no_check.unwrap_or_default();

      if let Some(ref cache_bl) = runtime.reload {
        if cache_bl.is_empty() {
          flags.reload = true;
        } else {
          flags.cache_blocklist = resolve_urls(cache_bl.clone());
          debug!("cache blocklist: {:#?}", &flags.cache_blocklist);
          flags.reload = false;
        }
      }

      flags.ca_file = runtime.cert.clone();

      if let Some(ref permissions) = runtime.permissions {
        if !runtime.extra.is_empty() {
          return Err(());
        }

        if permissions.all.unwrap_or_default() {
          flags.allow_read = true;
          flags.allow_env = true;
          flags.allow_net = true;
          flags.allow_run = true;
          flags.allow_read = true;
          flags.allow_write = true;
          flags.allow_plugin = true;
          flags.allow_hrtime = true;
        } else {
          if let Some(ref read_wl) = permissions.read {
            let read_allowlist: Vec<PathBuf> =
              read_wl.iter().map(PathBuf::from).collect();

            if read_allowlist.is_empty() {
              flags.allow_read = true;
            } else {
              flags.read_allowlist = read_allowlist;
            }
          }

          if let Some(ref write_wl) = permissions.write {
            let write_allowlist: Vec<PathBuf> =
              write_wl.iter().map(PathBuf::from).collect();

            if write_allowlist.is_empty() {
              flags.allow_write = true;
            } else {
              flags.write_allowlist = write_allowlist;
            }
          }

          if let Some(ref net_wl) = permissions.net {
            if net_wl.is_empty() {
              flags.allow_net = true;
            } else {
              flags.net_allowlist =
                crate::flags_allow_net::parse(net_wl.clone()).unwrap();
              debug!("net allowlist: {:#?}", &flags.net_allowlist);
            }
          }

          flags.allow_plugin = permissions.plugin.unwrap_or_default();
          flags.allow_run = permissions.run.unwrap_or_default();
          flags.allow_env = permissions.env.unwrap_or_default();
          flags.allow_hrtime = permissions.hrtime.unwrap_or_default();
        }
      }
    }

    if let Some(ref test) = self.test {
      if !test.extra.is_empty() {
        return Err(());
      }

      if !flags.unstable && test.no_run.unwrap_or_default() {
        return Err(());
      }
      if !flags.unstable && test.coverage.unwrap_or_default() {
        return Err(());
      }
    }
    if let Some(ref fmt) = self.fmt {
      if !fmt.extra.is_empty() {
        return Err(());
      }
    }
    if let Some(ref lint) = self.lint {
      if !lint.extra.is_empty() {
        return Err(());
      }

      if !flags.unstable && lint.ignore.is_some() {
        return Err(());
      }
    }

    Ok(flags)
  }
}
