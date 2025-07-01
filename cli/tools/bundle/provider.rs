use std::sync::Arc;

use deno_core::error::AnyError;
use deno_runtime::ops::bundle as rt_bundle;
use deno_runtime::ops::bundle::BundleOptions;
use deno_runtime::ops::bundle::BundleProvider;

use crate::args::DenoSubcommand;
use crate::args::Flags;

pub struct CliBundleProvider {
  flags: Arc<Flags>,
}

impl CliBundleProvider {
  pub fn new(flags: Arc<Flags>) -> Self {
    Self { flags }
  }
}

impl From<rt_bundle::BundleFormat> for crate::args::BundleFormat {
  fn from(value: rt_bundle::BundleFormat) -> Self {
    match value {
      rt_bundle::BundleFormat::Cjs => crate::args::BundleFormat::Cjs,
      rt_bundle::BundleFormat::Esm => crate::args::BundleFormat::Esm,
      rt_bundle::BundleFormat::Iife => crate::args::BundleFormat::Iife,
    }
  }
}

impl From<rt_bundle::BundlePlatform> for crate::args::BundlePlatform {
  fn from(value: rt_bundle::BundlePlatform) -> Self {
    match value {
      rt_bundle::BundlePlatform::Browser => {
        crate::args::BundlePlatform::Browser
      }
      rt_bundle::BundlePlatform::Deno => crate::args::BundlePlatform::Deno,
    }
  }
}

impl From<rt_bundle::PackageHandling> for crate::args::PackageHandling {
  fn from(value: rt_bundle::PackageHandling) -> Self {
    match value {
      rt_bundle::PackageHandling::Bundle => {
        crate::args::PackageHandling::Bundle
      }
      rt_bundle::PackageHandling::External => {
        crate::args::PackageHandling::External
      }
    }
  }
}

impl From<rt_bundle::SourceMapType> for crate::args::SourceMapType {
  fn from(value: rt_bundle::SourceMapType) -> Self {
    match value {
      rt_bundle::SourceMapType::Inline => crate::args::SourceMapType::Inline,
      rt_bundle::SourceMapType::External => {
        crate::args::SourceMapType::External
      }
      rt_bundle::SourceMapType::Linked => crate::args::SourceMapType::Linked,
    }
  }
}

impl From<BundleOptions> for crate::args::BundleFlags {
  fn from(value: BundleOptions) -> Self {
    Self {
      entrypoints: value.entrypoints,
      output_path: value.output_path,
      output_dir: value.output_dir,
      external: value.external,
      format: value.format.into(),
      minify: value.minify,
      code_splitting: value.code_splitting,
      platform: value.platform.into(),
      watch: value.watch,
      sourcemap: value.sourcemap.map(|s| s.into()),
      one_file: value.one_file,
      packages: value.packages.into(),
    }
  }
}

#[async_trait::async_trait]
impl BundleProvider for CliBundleProvider {
  async fn bundle(
    &self,
    options: deno_runtime::ops::bundle::BundleOptions,
  ) -> Result<(), AnyError> {
    let mut flags_clone = (*self.flags).clone();
    let bundle_flags: crate::args::BundleFlags = options.into();
    flags_clone.subcommand = DenoSubcommand::Bundle(bundle_flags.clone());
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
      deno_runtime::tokio_util::create_and_run_current_thread(async move {
        let flags = Arc::new(flags_clone);
        let result = super::bundle(flags, bundle_flags).await;
        let _ = tx.send(result);
      })
    });
    rx.await??;
    Ok(())
  }
}
