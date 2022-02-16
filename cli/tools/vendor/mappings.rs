// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Position;
use deno_graph::Resolved;

use crate::fs_util::path_with_stem_suffix;

use super::specifiers::dir_name_for_root;
use super::specifiers::get_unique_path;
use super::specifiers::make_url_relative;
use super::specifiers::partition_by_root_specifiers;
use super::specifiers::sanitize_filepath;

pub struct ProxiedModule {
  pub output_path: PathBuf,
  pub declaration_specifier: ModuleSpecifier,
}

/// Constructs and holds the remote specifier to local path mappings.
pub struct Mappings {
  output_dir: ModuleSpecifier,
  mappings: HashMap<ModuleSpecifier, PathBuf>,
  base_specifiers: Vec<ModuleSpecifier>,
  proxies: HashMap<ModuleSpecifier, ProxiedModule>,
}

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
    let mut proxies = HashMap::new();
    let mut base_specifiers = Vec::new();

    for (root, specifiers) in partitioned_specifiers.into_iter() {
      let base_dir = get_unique_path(
        output_dir.join(dir_name_for_root(&root)),
        &mut mapped_paths,
      );
      for specifier in specifiers {
        let media_type = graph.get(&specifier).unwrap().media_type;
        let sub_path = sanitize_filepath(&make_url_relative(&root, &{
          let mut specifier = specifier.clone();
          specifier.set_query(None);
          specifier
        })?);
        let new_path = path_with_extension(
          &base_dir.join(if cfg!(windows) {
            sub_path.replace('/', "\\")
          } else {
            sub_path
          }),
          &media_type.as_ts_extension()[1..],
        );
        mappings
          .insert(specifier, get_unique_path(new_path, &mut mapped_paths));
      }
      base_specifiers.push(root.clone());
      mappings.insert(root, base_dir);
    }

    // resolve all the "proxy" paths to use for when an x-typescript-types header is specified
    for module in remote_modules {
      if let Some((
        _,
        Resolved::Ok {
          specifier, range, ..
        },
      )) = &module.maybe_types_dependency
      {
        // hack to tell if it's an x-typescript-types header
        let is_ts_types_header =
          range.start == Position::zeroed() && range.end == Position::zeroed();
        if is_ts_types_header {
          let module_path = mappings.get(&module.specifier).unwrap();
          let proxied_path = get_unique_path(
            path_with_stem_suffix(module_path, ".proxied"),
            &mut mapped_paths,
          );
          proxies.insert(
            module.specifier.clone(),
            ProxiedModule {
              output_path: proxied_path,
              declaration_specifier: specifier.clone(),
            },
          );
        }
      }
    }

    Ok(Self {
      output_dir: ModuleSpecifier::from_directory_path(output_dir).unwrap(),
      mappings,
      base_specifiers,
      proxies,
    })
  }

  pub fn output_dir(&self) -> &ModuleSpecifier {
    &self.output_dir
  }

  pub fn local_uri(&self, specifier: &ModuleSpecifier) -> ModuleSpecifier {
    if specifier.scheme() == "file" {
      specifier.clone()
    } else {
      let local_path = self.local_path(specifier);
      if specifier.path().ends_with('/') {
        ModuleSpecifier::from_directory_path(&local_path)
      } else {
        ModuleSpecifier::from_file_path(&local_path)
      }
      .unwrap_or_else(|_| {
        panic!("Could not convert {} to uri.", local_path.display())
      })
    }
  }

  pub fn local_path(&self, specifier: &ModuleSpecifier) -> PathBuf {
    if specifier.scheme() == "file" {
      specifier.to_file_path().unwrap()
    } else {
      self
        .mappings
        .get(specifier)
        .as_ref()
        .unwrap_or_else(|| {
          panic!("Could not find local path for {}", specifier)
        })
        .to_path_buf()
    }
  }

  pub fn relative_path(
    &self,
    from: &ModuleSpecifier,
    to: &ModuleSpecifier,
  ) -> String {
    let mut from = self.local_uri(from);
    let to = self.local_uri(to);

    // workaround using parent directory until https://github.com/servo/rust-url/pull/754 is merged
    if !from.path().ends_with('/') {
      let local_path = self.local_path(&from);
      from = ModuleSpecifier::from_directory_path(local_path.parent().unwrap())
        .unwrap();
    }

    // workaround for url crate not adding a trailing slash for a directory
    // it seems to be fixed once a version greater than 2.2.2 is released
    let is_dir = to.path().ends_with('/');
    let mut text = from.make_relative(&to).unwrap();
    if is_dir && !text.ends_with('/') && to.query().is_none() {
      text.push('/');
    }
    text
  }

  pub fn relative_specifier_text(
    &self,
    from: &ModuleSpecifier,
    to: &ModuleSpecifier,
  ) -> String {
    let relative_path = self.relative_path(from, to);

    if relative_path.starts_with("../") || relative_path.starts_with("./") {
      relative_path
    } else {
      format!("./{}", relative_path)
    }
  }

  pub fn base_specifiers(&self) -> &Vec<ModuleSpecifier> {
    &self.base_specifiers
  }

  pub fn base_specifier(
    &self,
    child_specifier: &ModuleSpecifier,
  ) -> &ModuleSpecifier {
    self
      .base_specifiers
      .iter()
      .find(|s| child_specifier.as_str().starts_with(s.as_str()))
      .unwrap_or_else(|| {
        panic!("Could not find base specifier for {}", child_specifier)
      })
  }

  pub fn proxied_path(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    self.proxies.get(specifier).map(|s| s.output_path.clone())
  }

  pub fn proxied_modules(
    &self,
  ) -> std::collections::hash_map::Iter<'_, ModuleSpecifier, ProxiedModule> {
    self.proxies.iter()
  }
}

fn path_with_extension(path: &Path, new_ext: &str) -> PathBuf {
  if let Some(file_stem) = path.file_stem().map(|f| f.to_string_lossy()) {
    if let Some(old_ext) = path.extension().map(|f| f.to_string_lossy()) {
      if file_stem.to_lowercase().ends_with(".d") {
        if new_ext.to_lowercase() == format!("d.{}", old_ext.to_lowercase()) {
          // maintain casing
          return path.to_path_buf();
        }
        return path.with_file_name(format!(
          "{}.{}",
          &file_stem[..file_stem.len() - ".d".len()],
          new_ext
        ));
      }
      if new_ext.to_lowercase() == old_ext.to_lowercase() {
        // maintain casing
        return path.to_path_buf();
      }
      let media_type: MediaType = path.into();
      if media_type == MediaType::Unknown {
        return path.with_file_name(format!(
          "{}.{}",
          path.file_name().unwrap().to_string_lossy(),
          new_ext
        ));
      }
    }
  }
  path.with_extension(new_ext)
}

#[cfg(test)]
mod test {
  use pretty_assertions::assert_eq;

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
      // maintains casing
      PathBuf::from("/test.D.TS"),
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.TS"), "ts"),
      // maintains casing
      PathBuf::from("/test.TS"),
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.ts"), "js"),
      PathBuf::from("/test.js")
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/test.js"), "js"),
      PathBuf::from("/test.js")
    );
    assert_eq!(
      path_with_extension(&PathBuf::from("/chai@1.2.3"), "js"),
      PathBuf::from("/chai@1.2.3.js")
    );
  }
}
