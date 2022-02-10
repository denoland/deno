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

use super::specifiers::dir_name_for_root;
use super::specifiers::get_unique_path;
use super::specifiers::make_url_relative;
use super::specifiers::partition_by_root_specifiers;
use super::specifiers::sanitize_filepath;

/// Constructs and holds the remote specifier to local path mappings.
pub struct Mappings {
  output_dir: ModuleSpecifier,
  mappings: HashMap<ModuleSpecifier, PathBuf>,
  base_specifiers: Vec<ModuleSpecifier>,
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
    let mut base_specifiers = Vec::new();

    for (root, specifiers) in partitioned_specifiers.into_iter() {
      let base_dir = get_unique_path(
        output_dir.join(dir_name_for_root(&root, &specifiers)),
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

    Ok(Self {
      output_dir: ModuleSpecifier::from_directory_path(output_dir).unwrap(),
      mappings,
      base_specifiers,
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

    from.make_relative(&to).unwrap()
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
}

fn path_with_extension(path: &Path, ext: &str) -> PathBuf {
  if let Some(file_stem) = path.file_stem().map(|f| f.to_string_lossy()) {
    if path.extension().is_some() {
      if file_stem.to_lowercase().ends_with(".d") {
        return path.with_file_name(format!(
          "{}.{}",
          &file_stem[..file_stem.len() - ".d".len()],
          ext
        ));
      }
      let media_type: MediaType = path.into();
      if media_type == MediaType::Unknown {
        return path.with_file_name(format!(
          "{}.{}",
          path.file_name().unwrap().to_string_lossy(),
          ext
        ));
      }
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
    assert_eq!(
      path_with_extension(&PathBuf::from("/chai@1.2.3"), "js"),
      PathBuf::from("/chai@1.2.3.js")
    );
  }
}
