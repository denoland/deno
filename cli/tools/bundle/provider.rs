// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_bundle_runtime as rt_bundle;
use deno_bundle_runtime::BundleOptions as RtBundleOptions;
use deno_bundle_runtime::BundleProvider;
use deno_core::error::AnyError;

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

impl From<RtBundleOptions> for crate::args::BundleFlags {
  fn from(value: RtBundleOptions) -> Self {
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
    namespace: Some(location.namespace),
    line: location.line,
    column: location.column,
    length: Some(location.length),
    suggestion: Some(location.suggestion),
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
    options: RtBundleOptions,
  ) -> Result<rt_bundle::BuildResponse, AnyError> {
    let mut flags_clone = (*self.flags).clone();
    let write_output = options.write
      && (options.output_dir.is_some() || options.output_path.is_some());
    let bundle_flags: crate::args::BundleFlags = options.into();
    flags_clone.subcommand = DenoSubcommand::Bundle(bundle_flags.clone());
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
      deno_runtime::tokio_util::create_and_run_current_thread(async move {
        let flags = Arc::new(flags_clone);
        let bundler = match super::bundle_init(flags, &bundle_flags).await {
          Ok(bundler) => bundler,
          Err(e) => {
            log::trace!("bundle_init error: {e:?}");
            let _ = tx.send(Err(e));
            return Ok(());
          }
        };
        log::trace!("bundler.build");
        let mut result = match bundler.build().await {
          Ok(result) => result,
          Err(e) => {
            log::trace!("bundler.build error: {e:?}");
            let _ = tx.send(Err(e));
            return Ok(());
          }
        };
        log::trace!("process_result");
        if write_output {
          super::process_result(
            &result,
            &bundler.cwd,
            true,
            bundle_flags.minify,
          )?;
          result.output_files = None;
        }
        log::trace!("convert_build_response");
        let result = convert_build_response(result);
        log::trace!("send result");
        let _ = tx.send(Ok(result));
        Ok::<_, AnyError>(())
      })
    });
    log::trace!("rx.await");
    let response = rx.await??;
    log::trace!("response: {:?}", response);
    Ok(response)
  }
}
