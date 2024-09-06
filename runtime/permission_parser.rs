// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::normalize_path;

pub struct RuntimePermissionParser {
  initial_cwd: PathBuf,
  fs: deno_fs::FileSystemRc,
}

impl RuntimePermissionParser {
  fn resolve_from_cwd(&self, path: &Path) -> PathBuf {
    if path.is_absolute() {
      normalize_path(path)
    } else {
      normalize_path(self.initial_cwd.join(path))
    }
  }
}

impl deno_permissions::PermissionParser for RuntimePermissionParser {
  fn parse_read_descriptor(
    &self,
    arg: &str,
  ) -> Result<deno_permissions::ReadDescriptor, AnyError> {
    if arg.is_empty() {
      Err(AnyError::msg("Empty path is not allowed"))
    } else {
      Ok(self.resolve_from_cwd(Path::new(arg)))
    }
  }
}
