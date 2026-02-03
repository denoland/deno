// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::FromV8;
use deno_core::OpState;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_error::JsErrorBox;

deno_core::extension!(
  deno_bundle_runtime,
  deps = [
    deno_web
  ],
  ops = [
    op_bundle,
  ],
  esm = [
    "bundle.ts"
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
  async fn bundle(
    &self,
    _options: BundleOptions,
  ) -> Result<BuildResponse, AnyError> {
    Err(deno_core::anyhow::anyhow!(
      "default BundleProvider does not do anything"
    ))
  }
}

#[async_trait]
pub trait BundleProvider: Send + Sync {
  async fn bundle(
    &self,
    options: BundleOptions,
  ) -> Result<BuildResponse, AnyError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Default, FromV8)]
pub struct BundleOptions {
  pub entrypoints: Vec<String>,
  pub output_path: Option<String>,
  pub output_dir: Option<String>,
  #[from_v8(default)]
  pub external: Vec<String>,
  #[from_v8(serde, default)]
  pub format: BundleFormat,
  #[from_v8(default)]
  pub minify: bool,
  #[from_v8(default)]
  pub code_splitting: bool,
  #[from_v8(default = true)]
  pub inline_imports: bool,
  #[from_v8(serde, default)]
  pub packages: PackageHandling,
  #[from_v8(serde)]
  pub sourcemap: Option<SourceMapType>,
  #[from_v8(serde, default)]
  pub platform: BundlePlatform,
  #[from_v8(default = true)]
  pub write: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundlePlatform {
  Browser,
  #[default]
  Deno,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundleFormat {
  #[default]
  Esm,
  Cjs,
  Iife,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, FromV8, ToV8)]
pub struct Message {
  pub text: String,
  pub location: Option<Location>,
  pub notes: Vec<Note>,
}

#[derive(Debug, Clone, FromV8, ToV8)]
pub struct PartialMessage {
  pub id: Option<String>,
  pub plugin_name: Option<String>,
  pub text: Option<String>,
  pub location: Option<Location>,
  pub notes: Option<Vec<Note>>,
  pub detail: Option<u32>,
}

#[derive(Debug, Clone, ToV8)]
pub struct BuildOutputFile {
  pub path: String,
  pub contents: Option<Uint8Array>,
  pub hash: String,
}
#[derive(Debug, Clone, ToV8)]
pub struct BuildResponse {
  pub errors: Vec<Message>,
  pub warnings: Vec<Message>,
  pub output_files: Option<Vec<BuildOutputFile>>,
}
#[derive(Debug, Clone, FromV8, ToV8)]
pub struct Note {
  pub text: String,
  pub location: Option<Location>,
}
#[derive(Debug, Clone, FromV8, ToV8)]
pub struct Location {
  pub file: String,
  pub namespace: Option<String>,
  pub line: u32,
  pub column: u32,
  pub length: Option<u32>,
  pub suggestion: Option<String>,
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<regex::Regex, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::Deserialize;
  let s = String::deserialize(deserializer)?;
  regex::Regex::new(&s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnResolveOptions {
  #[serde(deserialize_with = "deserialize_regex")]
  pub filter: regex::Regex,
  pub namespace: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnLoadOptions {
  #[serde(deserialize_with = "deserialize_regex")]
  pub filter: regex::Regex,
  pub namespace: Option<String>,
}

#[op2]
pub async fn op_bundle(
  state: Rc<RefCell<OpState>>,
  #[scoped] options: BundleOptions,
) -> Result<BuildResponse, JsErrorBox> {
  // eprintln!("op_bundle: {:?}", options);
  let provider = {
    let state = state.borrow();
    state.borrow::<Arc<dyn BundleProvider>>().clone()
  };

  provider
    .bundle(options)
    .await
    .map_err(|e| JsErrorBox::generic(e.to_string()))
}
