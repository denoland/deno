// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_semver::SmallStackString;
use deno_semver::Version;
use jsonc_parser::cst::CstObject;
use jsonc_parser::cst::CstRootNode;
use jsonc_parser::json;
use log::info;

use crate::args::Flags;
use crate::args::VersionFlags;
use crate::args::VersionIncrement;
use crate::factory::CliFactory;

struct ConfigUpdater {
  cst: CstRootNode,
  root_object: CstObject,
  path: PathBuf,
  modified: bool,
}

impl ConfigUpdater {
  fn new(config_file_path: PathBuf) -> Result<Self, AnyError> {
    let config_file_contents = std::fs::read_to_string(&config_file_path)
      .with_context(|| {
        format!("Reading config file '{}'", config_file_path.display())
      })?;
    let cst = CstRootNode::parse(&config_file_contents, &Default::default())
      .with_context(|| {
        format!("Parsing config file '{}'", config_file_path.display())
      })?;
    let root_object = cst.object_value_or_set();
    Ok(Self {
      cst,
      root_object,
      path: config_file_path,
      modified: false,
    })
  }

  fn display_path(&self) -> String {
    deno_path_util::url_from_file_path(&self.path)
      .map(|u| u.to_string())
      .unwrap_or_else(|_| self.path.display().to_string())
  }

  fn get_version(&self) -> Option<String> {
    self
      .root_object
      .get("version")?
      .value()?
      .as_string_lit()
      .and_then(|s| s.decoded_value().ok())
  }

  fn set_version(&mut self, version: &str) {
    let version_prop = self.root_object.get("version");
    match version_prop {
      Some(prop) => {
        prop.set_value(json!(version));
        self.modified = true;
      }
      None => {
        // Insert the version property at the beginning for better organization
        self.root_object.insert(0, "version", json!(version));
        self.modified = true;
      }
    }
  }

  fn commit(&self) -> Result<(), AnyError> {
    if !self.modified {
      return Ok(());
    }

    let new_text = self.cst.to_string();
    std::fs::write(&self.path, new_text).with_context(|| {
      format!("failed writing to '{}'", self.path.display())
    })?;
    Ok(())
  }
}

fn increment_version(
  current: &Version,
  increment: &VersionIncrement,
) -> Version {
  match increment {
    VersionIncrement::Major => Version {
      major: current.major + 1,
      minor: 0,
      patch: 0,
      pre: Default::default(),
      build: Default::default(),
    },
    VersionIncrement::Minor => Version {
      major: current.major,
      minor: current.minor + 1,
      patch: 0,
      pre: Default::default(),
      build: Default::default(),
    },
    VersionIncrement::Patch => Version {
      major: current.major,
      minor: current.minor,
      patch: current.patch + 1,
      pre: Default::default(),
      build: Default::default(),
    },
    VersionIncrement::Premajor => {
      let mut v = Version {
        major: current.major + 1,
        minor: 0,
        patch: 0,
        ..Default::default()
      };
      v.pre = vec![SmallStackString::from_static("0")].into();
      v
    }
    VersionIncrement::Preminor => {
      let mut v = Version {
        major: current.major,
        minor: current.minor + 1,
        patch: 0,
        ..Default::default()
      };
      v.pre = vec![SmallStackString::from_static("0")].into();
      v
    }
    VersionIncrement::Prepatch => {
      let mut v = Version {
        major: current.major,
        minor: current.minor,
        patch: current.patch + 1,
        ..Default::default()
      };
      v.pre = vec![SmallStackString::from_static("0")].into();
      v
    }
    VersionIncrement::Prerelease => {
      let mut v = current.clone();
      if v.pre.is_empty() {
        v.patch += 1;
        v.pre = vec![SmallStackString::from_static("0")].into();
      } else {
        let mut pre_vec = v.pre.iter().cloned().collect::<Vec<_>>();
        if let Some(last) = pre_vec.last_mut() {
          if let Ok(num) = last.parse::<u64>() {
            *last = SmallStackString::from_string((num + 1).to_string());
          } else {
            pre_vec.push(SmallStackString::from_static("0"));
          }
        }
        v.pre = pre_vec.into();
      }
      v
    }
  }
}

fn load_config(
  cli_options: &crate::args::CliOptions,
) -> Result<ConfigUpdater, AnyError> {
  let start_dir = &cli_options.start_dir;

  // Check for deno.json first - it takes priority
  if let Some(deno_json) = start_dir.member_deno_json() {
    let config_path = deno_path_util::url_to_file_path(&deno_json.specifier)
      .context("Failed to convert deno.json URL to path")?;
    return ConfigUpdater::new(config_path);
  } else if let Some(pkg_json) = start_dir.member_pkg_json() {
    // Only fall back to package.json if deno.json doesn't exist
    return ConfigUpdater::new(pkg_json.path.clone());
  }

  bail!("No deno.json or package.json found in the current directory")
}

pub fn bump_version_command(
  flags: Arc<Flags>,
  version_flags: VersionFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  let mut config = load_config(cli_options)?;

  let current_version = if let Some(version_str) = config.get_version() {
    Version::parse_standard(&version_str).with_context(|| {
      format!(
        "Failed to parse version '{}' in {}",
        version_str,
        config.display_path()
      )
    })?
  } else {
    if version_flags.increment.is_none() {
      println!("No version found in configuration file");
      return Ok(());
    }
    // Default to 0.1.0 if no version is found but increment is specified
    info!("No version found, defaulting to 0.1.0");
    Version::parse_standard("0.1.0")
      .with_context(|| "Failed to create default version")?
  };

  let new_version = match &version_flags.increment {
    Some(increment) => increment_version(&current_version, increment),
    None => {
      println!("{}", current_version);
      return Ok(());
    }
  };

  config.set_version(&new_version.to_string());
  config.commit()?;

  println!("{}", new_version);
  info!(
    "Version updated from {} to {} in {}",
    current_version,
    new_version,
    config.display_path()
  );
  Ok(())
}
