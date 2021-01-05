// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_runtime::permissions::PermissionsOptions;
use log::Level;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Default, Deserialize, Serialize)]
pub struct Flags {
  /// Vector of CLI arguments - these are user script arguments, all Deno
  /// specific flags are removed.
  pub argv: Vec<String>,
  pub subcommand: DenoSubcommand,

  pub allow_env: bool,
  pub allow_hrtime: bool,
  pub allow_net: Option<Vec<String>>,
  pub allow_plugin: bool,
  pub allow_read: Option<Vec<PathBuf>>,
  pub allow_run: bool,
  pub allow_write: Option<Vec<PathBuf>>,
  pub cache_blocklist: Vec<String>,
  pub ca_file: Option<String>,
  pub cached_only: bool,
  pub config_path: Option<String>,
  pub coverage_dir: Option<String>,
  pub ignore: Vec<PathBuf>,
  pub import_map_path: Option<String>,
  pub inspect: Option<SocketAddr>,
  pub inspect_brk: Option<SocketAddr>,
  pub lock: Option<PathBuf>,
  pub lock_write: bool,
  #[serde(deserialize_with = "deserialize_maybe_log_level")]
  #[serde(serialize_with = "serialize_maybe_log_level")]
  pub log_level: Option<Level>,
  pub no_check: bool,
  pub no_prompts: bool,
  pub no_remote: bool,
  pub reload: bool,
  pub repl: bool,
  pub seed: Option<u64>,
  pub unstable: bool,
  pub v8_flags: Vec<String>,
  pub version: bool,
  pub watch: bool,
}

impl From<Flags> for PermissionsOptions {
  fn from(flags: Flags) -> Self {
    Self {
      allow_env: flags.allow_env,
      allow_hrtime: flags.allow_hrtime,
      allow_net: flags.allow_net,
      allow_plugin: flags.allow_plugin,
      allow_read: flags.allow_read,
      allow_run: flags.allow_run,
      allow_write: flags.allow_write,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum DenoSubcommand {
  Bundle {
    source_file: String,
    out_file: Option<PathBuf>,
  },
  Cache {
    files: Vec<String>,
  },
  Compile {
    source_file: String,
    output: Option<PathBuf>,
    args: Vec<String>,
  },
  Completions {
    buf: Box<[u8]>,
  },
  Doc {
    private: bool,
    json: bool,
    source_file: Option<String>,
    filter: Option<String>,
  },
  Eval {
    print: bool,
    code: String,
    as_typescript: bool,
  },
  Fmt {
    check: bool,
    files: Vec<PathBuf>,
    ignore: Vec<PathBuf>,
  },
  Info {
    json: bool,
    file: Option<String>,
  },
  Install {
    module_url: String,
    args: Vec<String>,
    name: Option<String>,
    root: Option<PathBuf>,
    force: bool,
  },
  LanguageServer,
  Lint {
    files: Vec<PathBuf>,
    ignore: Vec<PathBuf>,
    rules: bool,
    json: bool,
  },
  Repl,
  Run {
    script: String,
  },
  Test {
    no_run: bool,
    fail_fast: bool,
    quiet: bool,
    allow_none: bool,
    include: Option<Vec<String>>,
    filter: Option<String>,
  },
  Types,
  Upgrade {
    dry_run: bool,
    force: bool,
    canary: bool,
    version: Option<String>,
    output: Option<PathBuf>,
    ca_file: Option<String>,
  },
}

impl Default for DenoSubcommand {
  fn default() -> DenoSubcommand {
    DenoSubcommand::Repl
  }
}

fn deserialize_maybe_log_level<'de, D>(d: D) -> Result<Option<Level>, D::Error>
where
  D: Deserializer<'de>,
{
  struct OptionalLogLevelVisitor;
  impl<'de> de::Visitor<'de> for OptionalLogLevelVisitor {
    type Value = Option<Level>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      write!(formatter, "null or a valid log level string")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
    where
      D: de::Deserializer<'de>,
    {
      struct LogLevelVisitor;
      impl<'de> de::Visitor<'de> for LogLevelVisitor {
        type Value = Level;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
          write!(formatter, "a valid log level string")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
          E: de::Error,
        {
          Level::from_str(s).map_err(|_| {
            de::Error::invalid_value(de::Unexpected::Str(s), &self)
          })
        }
      }
      Ok(Some(d.deserialize_str(LogLevelVisitor)?))
    }
  }
  d.deserialize_option(OptionalLogLevelVisitor)
}

fn serialize_maybe_log_level<S>(
  maybe_level: &Option<Level>,
  s: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  match maybe_level {
    None => s.serialize_none(),
    Some(level) => s.serialize_str(&level.to_string()),
  }
}
