// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::{ConfiguresFiles, Filters, FiltersFiles};
use crate::util::path::specifier_to_file_path;

use deno_core::error::{generic_error, AnyError};

/// Collect included and ignored files. CLI flags take precedence
/// over config file, i.e. if there's `files.ignore` in config file
/// and `--ignore` CLI flag, only the flag value is taken into account.
pub fn collect_filters<T, U>(
  flags: &T,
  maybe_config: &Option<U>,
) -> Result<Filters, AnyError>
where
  T: FiltersFiles,
  U: ConfiguresFiles,
{
  let filters = flags.get_filters();

  let mut include = filters.include;
  let mut ignore = filters.ignore;

  if let Some(config) = maybe_config {
    if include.is_empty() {
      include = config
        .get_files_config()
        .include
        .iter()
        .filter_map(|s| specifier_to_file_path(s).ok())
        .collect::<Vec<_>>();
    }

    if ignore.is_empty() {
      ignore = config
        .get_files_config()
        .exclude
        .iter()
        .filter_map(|s| specifier_to_file_path(s).ok())
        .collect::<Vec<_>>();
    }
  }

  // Default to current working dir when include is not set as flag or
  // config file field.
  if include.is_empty() {
    include.push(match std::env::current_dir() {
      Ok(cwd) => cwd,
      Err(e) => return Err(generic_error(e.to_string())),
    });
  }

  Ok(Filters { include, ignore })
}
