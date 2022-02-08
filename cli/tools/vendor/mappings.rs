// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_graph::Module;
use deno_graph::ModuleGraph;

use super::specifiers::dir_name_for_root;
use super::specifiers::get_unique_path;
use super::specifiers::make_url_relative;
use super::specifiers::partition_by_root_specifiers;
use super::specifiers::sanitize_filepath;

/// Constructs and holds the remote specifier to local path mappings.
pub struct Mappings(HashMap<ModuleSpecifier, PathBuf>);

impl Mappings {
  pub fn from_remote_modules(
    graph: &ModuleGraph,
    remote_modules: &[&Module],
    output_dir: &Path,
  ) -> Result<Self, AnyError> {
    let partitioned_specifiers =
      partition_by_root_specifiers(remote_modules.iter().map(|m| &m.specifier));
    let mut mapped_paths = HashSet::new();
    let mut mappings = HashMap::new();

    for (root, specifiers) in partitioned_specifiers.into_iter() {
      let base_dir = get_unique_path(
        output_dir.join(dir_name_for_root(&root, &specifiers)),
        &mut mapped_paths,
      );
      for specifier in specifiers {
        let media_type = graph.get(&specifier).unwrap().media_type;
        let new_path = path_with_extension(
          &base_dir
            .join(sanitize_filepath(&make_url_relative(&root, &specifier)?)),
          &media_type.as_ts_extension()[1..],
        );
        mappings
          .insert(specifier, get_unique_path(new_path, &mut mapped_paths));
      }
    }

    Ok(Self(mappings))
  }

  pub fn local_path(&self, specifier: &ModuleSpecifier) -> &PathBuf {
    self
      .0
      .get(specifier)
      .as_ref()
      .unwrap_or_else(|| panic!("Could not find local path for {}", specifier))
  }
}

fn path_with_extension(path: &Path, ext: &str) -> PathBuf {
  if let Some(file_stem) = path.file_stem().map(|f| f.to_string_lossy()) {
    if path.extension().is_some() && file_stem.to_lowercase().ends_with(".d") {
      return path.with_file_name(format!(
        "{}.{}",
        &file_stem[..file_stem.len() - ".d".len()],
        ext
      ));
    }
  }
  path.with_extension(ext)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_path_with_extension() {
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.D.TS"), "ts"),
      PathBuf::from("/test.ts")
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.D.MTS"), "js"),
      PathBuf::from("/test.js")
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.D.TS"), "d.ts"),
      PathBuf::from("/test.d.ts")
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.ts"), "js"),
      PathBuf::from("/test.js")
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.js"), "js"),
      PathBuf::from("/test.js")
    );
  }
}
