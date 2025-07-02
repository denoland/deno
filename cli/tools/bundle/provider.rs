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
      watch: false,
      sourcemap: value.sourcemap.map(|s| s.into()),
      inline_imports: value.inline_imports,
      packages: value.packages.into(),
    }
  }
}

fn convert_note(note: esbuild_client::protocol::Note) -> rt_bundle::Note {
  rt_bundle::Note {
    text: note.text,
    location: note.location.map(convert_location),
  }
}

fn convert_location(
  location: esbuild_client::protocol::Location,
) -> rt_bundle::Location {
  rt_bundle::Location {
    file: location.file,
    namespace: location.namespace,
    line: location.line,
    column: location.column,
    length: location.length,
    suggestion: location.suggestion,
  }
}
fn convert_message(
  message: esbuild_client::protocol::Message,
) -> rt_bundle::Message {
  rt_bundle::Message {
    text: message.text,
    location: message.location.map(convert_location),
    notes: message.notes.into_iter().map(convert_note).collect(),
  }
}

fn convert_build_output_file(
  file: esbuild_client::protocol::BuildOutputFile,
) -> rt_bundle::BuildOutputFile {
  rt_bundle::BuildOutputFile {
    path: file.path,
    contents: Some(file.contents),
    hash: file.hash,
  }
}

pub fn convert_build_response(
  response: esbuild_client::protocol::BuildResponse,
) -> rt_bundle::BuildResponse {
  rt_bundle::BuildResponse {
    errors: response.errors.into_iter().map(convert_message).collect(),
    warnings: response.warnings.into_iter().map(convert_message).collect(),
    output_files: response
      .output_files
      .map(|files| files.into_iter().map(convert_build_output_file).collect()),
  }
}

#[async_trait::async_trait]
impl BundleProvider for CliBundleProvider {
  async fn bundle(
    &self,
    options: deno_runtime::ops::bundle::BundleOptions,
  ) -> Result<rt_bundle::BuildResponse, AnyError> {
    let mut flags_clone = (*self.flags).clone();
    let bundle_flags: crate::args::BundleFlags = options.into();
    flags_clone.subcommand = DenoSubcommand::Bundle(bundle_flags.clone());
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
      deno_runtime::tokio_util::create_and_run_current_thread(async move {
        let flags = Arc::new(flags_clone);
        let write_output = bundle_flags.output_dir.is_some()
          || bundle_flags.output_path.is_some();
        let bundler = super::bundle_init(flags, &bundle_flags).await?;
        let mut result = match bundler.build().await {
          Ok(result) => result,
          Err(e) => {
            let _ = tx.send(Err(e));
            return Ok(());
          }
        };
        if write_output {
          super::process_result(&result, &bundler.cwd, true)?;
          result.output_files = None;
        }
        let result = convert_build_response(result);
        let _ = tx.send(Ok(result));
        Ok::<_, AnyError>(())
      })
    });
    let response = rx.await??;
    Ok(response)
  }
}
