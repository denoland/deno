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
        let relative = base_dir
          .join(sanitize_filepath(&make_url_relative(&root, &specifier)?))
          .with_extension(&media_type.as_ts_extension()[1..]);
        mappings
          .insert(specifier, get_unique_path(relative, &mut mapped_paths));
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
