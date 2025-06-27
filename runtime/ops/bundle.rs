use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_error::JsErrorBox;

deno_core::extension!(
  deno_bundle_runtime,
  ops = [
    op_bundle,
  ],
  options = {
    bundle_provider: Option<Arc<dyn BundleProvider>>,
  },
  state = |state, options| {
    if let Some(bundle_provider) = options.bundle_provider {
      state.put(bundle_provider);
    } else {
      state.put::<Arc<dyn BundleProvider>>(Arc::new(()));
    }
  },
);

#[async_trait]
impl BundleProvider for () {
  async fn bundle(&self, options: BundleOptions) -> Result<(), AnyError> {
    todo!()
  }
}

#[async_trait]
pub trait BundleProvider: Send + Sync {
  async fn bundle(&self, options: BundleOptions) -> Result<(), AnyError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Default, serde::Deserialize)]
pub struct BundleOptions {
  pub entrypoints: Vec<String>,
  #[serde(default)]
  pub output_path: Option<String>,
  #[serde(default)]
  pub output_dir: Option<String>,
  #[serde(default)]
  pub external: Vec<String>,
  #[serde(default)]
  pub format: BundleFormat,
  #[serde(default)]
  pub minify: bool,
  #[serde(default)]
  pub code_splitting: bool,
  #[serde(default)]
  pub one_file: bool,
  #[serde(default)]
  pub packages: PackageHandling,
  #[serde(default)]
  pub sourcemap: Option<SourceMapType>,
  #[serde(default)]
  pub platform: BundlePlatform,
  #[serde(default)]
  pub watch: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
pub enum BundlePlatform {
  Browser,
  #[default]
  Deno,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
pub enum BundleFormat {
  #[default]
  Esm,
  Cjs,
  Iife,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
pub enum SourceMapType {
  #[default]
  Linked,
  Inline,
  External,
}

impl std::fmt::Display for BundleFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      BundleFormat::Esm => write!(f, "esm"),
      BundleFormat::Cjs => write!(f, "cjs"),
      BundleFormat::Iife => write!(f, "iife"),
    }
  }
}

impl std::fmt::Display for SourceMapType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SourceMapType::Linked => write!(f, "linked"),
      SourceMapType::Inline => write!(f, "inline"),
      SourceMapType::External => write!(f, "external"),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
pub enum PackageHandling {
  #[default]
  Bundle,
  External,
}

impl std::fmt::Display for PackageHandling {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PackageHandling::Bundle => write!(f, "bundle"),
      PackageHandling::External => write!(f, "external"),
    }
  }
}

#[op2(async)]
pub async fn op_bundle(
  state: Rc<RefCell<OpState>>,
  #[serde] options: BundleOptions,
) -> Result<(), JsErrorBox> {
  let state = state.borrow();
  let provider = state.borrow::<Arc<dyn BundleProvider>>().clone();
  drop(state);
  provider
    .bundle(options)
    .await
    .map_err(|e| JsErrorBox::generic(e.to_string()))
}
