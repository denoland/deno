use deno_core::error::AnyError;

use serde::Deserialize;
use std::default::Default;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;
use toml::from_str;

#[derive(Deserialize)]
struct ProjectConfig {
  deno: Option<Config>,
}

#[derive(Default, Deserialize)]
pub struct Config {
  pub fmt: Option<FormatConfig>,
  pub lint: Option<LintConfig>,
}

#[derive(Deserialize)]
pub struct FormatConfig {
  #[serde(default)]
  pub ignore: Vec<String>,
}

#[derive(Deserialize)]
pub struct LintConfig {
  #[serde(default)]
  pub ignore: Vec<String>,
}

pub fn load(path: &str) -> Result<Config, AnyError> {
  from_str(&read_to_string(path)?)
    .map(|c: ProjectConfig| c.deno.unwrap_or_default())
    .map_err(AnyError::new)
}

pub fn relative_paths(config: &str, paths: &[String]) -> Vec<PathBuf> {
  let base_path = Path::new(config).parent().unwrap();
  paths
    .iter()
    .map(|p| {
      let mut new_path = base_path.to_path_buf();
      new_path.push(p);
      new_path
    })
    .collect()
}
