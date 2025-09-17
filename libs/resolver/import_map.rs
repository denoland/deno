// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;

use anyhow::Context;
use deno_config::workspace::WorkspaceRc;

#[derive(Debug, Clone)]
pub struct ExternalImportMap {
  pub path: PathBuf,
  pub value: serde_json::Value,
}

impl ExternalImportMap {
  fn load<TSys: sys_traits::FsRead>(
    sys: &TSys,
    path: PathBuf,
  ) -> Result<Self, anyhow::Error> {
    let contents = sys.fs_read_to_string(&path).with_context(|| {
      format!("Unable to read import map at '{}'", path.display())
    })?;
    let value = serde_json::from_str(&contents)?;
    Ok(Self { path, value })
  }
}

#[allow(clippy::disallowed_types)]
pub type WorkspaceExternalImportMapLoaderRc<TSys> =
  deno_maybe_sync::MaybeArc<WorkspaceExternalImportMapLoader<TSys>>;

#[derive(Debug)]
pub struct WorkspaceExternalImportMapLoader<TSys: sys_traits::FsRead> {
  sys: TSys,
  workspace: WorkspaceRc,
  maybe_external_import_maps: once_cell::sync::OnceCell<Vec<ExternalImportMap>>,
}

impl<TSys: sys_traits::FsRead> WorkspaceExternalImportMapLoader<TSys> {
  pub fn new(sys: TSys, workspace: WorkspaceRc) -> Self {
    Self {
      sys,
      workspace,
      maybe_external_import_maps: Default::default(),
    }
  }

  pub fn get_or_load(&self) -> Result<&Vec<ExternalImportMap>, anyhow::Error> {
    self
      .maybe_external_import_maps
      .get_or_try_init(|| {
        let Some(deno_json) = self.workspace.root_deno_json() else {
          return Ok(vec![]);
        };
        deno_json
          .to_import_map_paths()?
          .into_iter()
          .map(|path| ExternalImportMap::load(&self.sys, path))
          .collect()
      })
      .map(|v| v.as_ref())
  }
}
