// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

pub type DenoRtNativeAddonLoaderRc = Arc<dyn DenoRtNativeAddonLoader>;

pub trait DenoRtNativeAddonLoader: Send + Sync {
  fn load_if_in_vfs(&self, path: &Path) -> Option<Cow<'static, [u8]>>;

  fn load_and_resolve_path<'a>(
    &self,
    path: &'a Path,
  ) -> std::io::Result<Cow<'a, Path>> {
    match self.load_if_in_vfs(&path) {
      Some(bytes) => {
        // todo(THIS PR): cache this between calls, error on
        // write, and use a hash of the file name instead. Also,
        // use an "atomic write" and handle if another process is
        // using the file already (maybe read it and compare the bytes)
        let path = std::env::temp_dir().join(format!(
          "deno_rt_napi_{}{}",
          bytes.len(),
          path
            .extension()
            .map(|e| e.to_string_lossy())
            .unwrap_or_else(|| "".into())
        ));
        std::fs::write(&path, bytes).unwrap();
        Ok(Cow::Owned(path))
      }
      None => Ok(Cow::Borrowed(&path)),
    }
  }
}
