// Copyright 2018-2025 the Deno authors. MIT license.

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
use log::warn;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::VersionFlags;
use crate::args::VersionIncrement;
use crate::factory::CliFactory;

#[derive(Debug, Copy, Clone, Hash)]
enum ConfigKind {
  DenoJson,
  PackageJson,
}

struct ConfigUpdater {
  _kind: ConfigKind,
  cst: CstRootNode,
  root_object: CstObject,
  path: PathBuf,
  modified: bool,
}

impl ConfigUpdater {
  fn new(
    kind: ConfigKind,
    config_file_path: PathBuf,
  ) -> Result<Self, AnyError> {
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
      _kind: kind,
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

  fn contents(&self) -> String {
    self.cst.to_string()
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

    let new_text = self.contents();
    std::fs::write(&self.path, new_text).with_context(|| {
      format!("failed writing to '{}'", self.path.display())
    })?;
    Ok(())
  }
}

fn increment_version(
  current: &Version,
  increment: &VersionIncrement,
) -> Result<Version, AnyError> {
  let new_version = match increment {
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

    VersionIncrement::Premajor
    | VersionIncrement::Preminor
    | VersionIncrement::Prepatch => {
      let mut version = match increment {
        VersionIncrement::Premajor => Version {
          major: current.major + 1,
          minor: 0,
          patch: 0,
          ..Default::default()
        },
        VersionIncrement::Preminor => Version {
          major: current.major,
          minor: current.minor + 1,
          patch: 0,
          ..Default::default()
        },
        VersionIncrement::Prepatch => Version {
          major: current.major,
          minor: current.minor,
          patch: current.patch + 1,
          ..Default::default()
        },
        _ => unreachable!(),
      };
      version.pre = vec![SmallStackString::from_static("0")].into();
      version
    }

    VersionIncrement::Prerelease => {
      let mut new_version = current.clone();
      if new_version.pre.is_empty() {
        new_version.patch += 1;
        new_version.pre = vec![SmallStackString::from_static("0")].into();
      } else {
        let mut pre_vec = new_version.pre.iter().cloned().collect::<Vec<_>>();
        if let Some(last) = pre_vec.last_mut() {
          if let Ok(num) = last.parse::<u64>() {
            *last = SmallStackString::from_string((num + 1).to_string());
          } else {
            pre_vec.push(SmallStackString::from_static("0"));
          }
        }
        new_version.pre = pre_vec.into();
      }
      new_version
    }
  };

  Ok(new_version)
}

fn find_config_files(
  cli_options: &CliOptions,
) -> Result<Vec<ConfigUpdater>, AnyError> {
  let start_dir = &cli_options.start_dir;
  let mut configs = Vec::new();

  // Check for deno.json
  if let Some(deno_json) = start_dir.maybe_deno_json() {
    let config_path = deno_path_util::url_to_file_path(&deno_json.specifier)
      .context("Failed to convert deno.json URL to path")?;
    configs.push(ConfigUpdater::new(ConfigKind::DenoJson, config_path)?);
  }

  // Check for package.json
  if let Some(pkg_json) = start_dir.maybe_pkg_json() {
    configs.push(ConfigUpdater::new(
      ConfigKind::PackageJson,
      pkg_json.path.clone(),
    )?);
  }

  if configs.is_empty() {
    bail!("No deno.json or package.json found in the current directory");
  }

  Ok(configs)
}

fn create_git_tag(version: &str) -> Result<(), AnyError> {
  let output = std::process::Command::new("git")
    .args(["tag", &format!("v{}", version)])
    .output()
    .context("Failed to execute git tag command")?;

  if !output.status.success() {
    bail!(
      "Failed to create git tag: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }

  Ok(())
}

fn commit_version_changes(version: &str) -> Result<(), AnyError> {
  // Stage all changes
  let output = std::process::Command::new("git")
    .args(["add", "-A"])
    .output()
    .context("Failed to execute git add command")?;

  if !output.status.success() {
    bail!(
      "Failed to stage changes: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }

  // Commit changes
  let commit_msg = format!("chore: bump version to {}", version);
  let output = std::process::Command::new("git")
    .args(["commit", "-m", &commit_msg])
    .output()
    .context("Failed to execute git commit command")?;

  if !output.status.success() {
    bail!(
      "Failed to commit changes: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }

  Ok(())
}

pub fn version_command(
  flags: Arc<Flags>,
  version_flags: VersionFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  // Find and load config files
  let mut configs = find_config_files(cli_options)?;

  // Find the current version from the first config file that has one
  let mut current_version = None;
  for config in &configs {
    if let Some(version_str) = config.get_version() {
      current_version =
        Some(Version::parse_standard(&version_str).with_context(|| {
          format!(
            "Failed to parse version '{}' in {}",
            version_str,
            config.display_path()
          )
        })?);
      break;
    }
  }

  let current_version = match current_version {
    Some(v) => v,
    None => {
      if version_flags.increment.is_none() {
        // If no increment specified and no version found, just show current state
        info!("No version found in configuration files");
        return Ok(());
      }
      // Default to 1.0.0 if no version is found but increment is specified
      Version::parse_standard("1.0.0")
        .with_context(|| "Failed to create default version")?
    }
  };

  let new_version = match &version_flags.increment {
    Some(increment) => increment_version(&current_version, increment)?,
    None => {
      // Just show the current version
      info!("{}", current_version);
      return Ok(());
    }
  };

  if version_flags.dry_run {
    info!("Current version: {}", current_version);
    info!("New version: {}", new_version);
    for config in &configs {
      info!("Would update: {}", config.display_path());
    }
    return Ok(());
  }

  // Update version in all config files
  for config in &mut configs {
    config.set_version(&new_version.to_string());
    config.commit()?;
    info!("Updated version in {}", config.display_path());
  }

  // Handle git operations
  if version_flags.git_commit_all {
    commit_version_changes(&new_version.to_string())?;
    info!("Committed version changes");
  }

  if !version_flags.no_git_tag {
    if let Err(e) = create_git_tag(&new_version.to_string()) {
      warn!("Failed to create git tag: {}", e);
    } else {
      info!("Created git tag v{}", new_version);
    }
  }

  info!(
    "Version updated from {} to {}",
    current_version, new_version
  );
  Ok(())
}
