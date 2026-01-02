// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;

use anyhow::Context;
use deno_config::workspace::WorkspaceRc;

#[derive(Debug, Clone)]
pub struct ExternalImportMap {
  pub path: PathBuf,
  pub value: serde_json::Value,
}

#[allow(clippy::disallowed_types)]
pub type WorkspaceExternalImportMapLoaderRc<TSys> =
  deno_maybe_sync::MaybeArc<WorkspaceExternalImportMapLoader<TSys>>;

#[derive(Debug)]
pub struct WorkspaceExternalImportMapLoader<TSys: sys_traits::FsRead> {
  sys: TSys,
  workspace: WorkspaceRc,
  maybe_external_import_map:
    once_cell::sync::OnceCell<Option<ExternalImportMap>>,
}

impl<TSys: sys_traits::FsRead> WorkspaceExternalImportMapLoader<TSys> {
  pub fn new(sys: TSys, workspace: WorkspaceRc) -> Self {
    Self {
      sys,
      workspace,
      maybe_external_import_map: Default::default(),
    }
  }

  pub fn get_or_load(
    &self,
  ) -> Result<Option<&ExternalImportMap>, anyhow::Error> {
    self
      .maybe_external_import_map
      .get_or_try_init(|| {
        let Some(deno_json) = self.workspace.root_deno_json() else {
          return Ok(None);
        };
        if deno_json.is_an_import_map() {
          return Ok(None);
        }
        let Some(path) = deno_json.to_import_map_path()? else {
          return Ok(None);
        };
        let contents =
          self.sys.fs_read_to_string(&path).with_context(|| {
            format!("Unable to read import map at '{}'", path.display())
          })?;
        let value = serde_json::from_str(&contents)?;
        Ok(Some(ExternalImportMap { path, value }))
      })
      .map(|v| v.as_ref())
  }
}
