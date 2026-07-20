// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::sync::Arc;

use deno_bundle_runtime as rt_bundle;
use deno_bundle_runtime::BundleOptions as RtBundleOptions;
use deno_bundle_runtime::BundleProvider;
use deno_core::error::AnyError;
use rolldown::BundleOutput;
use rolldown_error::Severity;

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

fn convert_diagnostic(
  diag: &rolldown_error::BuildDiagnostic,
) -> rt_bundle::Message {
  rt_bundle::Message {
    text: diag.to_string(),
    location: None,
    notes: vec![],
  }
}

fn hash_contents(contents: &[u8]) -> String {
  use base64::prelude::*;
  let hash = twox_hash::XxHash64::oneshot(0, contents);
  let bytes = hash.to_le_bytes();
  base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes)
}

fn convert_bundle_output(
  bundler: &super::RolldownBundler,
  output: BundleOutput,
) -> rt_bundle::BuildResponse {
  // Error-level diagnostics (including unresolved imports) are rendered with
  // the same rich messages `deno bundle` prints.
  let errors = super::bundle_output_error_messages(bundler, &output);
  let warnings = output
    .warnings
    .iter()
    .filter(|diag| diag.severity() == Severity::Warning)
    .map(convert_diagnostic)
    .collect();

  // Match the `deno bundle` CLI: rewrite rolldown's `require` shim to
  // `createRequire` for JS chunks on the Deno platform. The write path does
  // this in `process_result`; the in-memory path must do it here too.
  let replace_require = super::should_replace_require_shim(bundler.platform);
  let output_files: Vec<rt_bundle::BuildOutputFile> = output
    .assets
    .iter()
    .map(|asset| {
      let filename = asset.filename().to_string();
      let mut contents = asset.content_as_bytes().to_vec();
      if replace_require
        && super::is_js(Path::new(&filename))
        && let Ok(text) = std::str::from_utf8(&contents)
      {
        contents =
          super::replace_require_shim(text, bundler.minified).into_bytes();
      }
      let hash = hash_contents(&contents);
      rt_bundle::BuildOutputFile {
        path: filename,
        contents: Some(contents.into()),
        hash,
      }
    })
    .collect();

  rt_bundle::BuildResponse {
    errors,
    warnings,
    output_files: Some(output_files),
  }
}

#[async_trait::async_trait]
impl BundleProvider for CliBundleProvider {
  async fn bundle(
    &self,
    options: RtBundleOptions,
    plugins: Option<rt_bundle::PluginHookTx>,
  ) -> Result<rt_bundle::BuildResponse, AnyError> {
    let mut flags_clone = (*self.flags).clone();
    flags_clone.type_check_mode = crate::args::TypeCheckMode::None;
    let write_output = options.write
      && (options.output_dir.is_some() || options.output_path.is_some());
    let bundle_flags: crate::args::BundleFlags = options.into();
    flags_clone.subcommand = DenoSubcommand::Bundle(bundle_flags.clone());
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
      deno_runtime::tokio_util::create_and_run_current_thread(async move {
        let flags = Arc::new(flags_clone);
        let bundler =
          match super::bundle_init(flags, &bundle_flags, plugins).await {
            Ok(bundler) => bundler,
            Err(e) => {
              log::trace!("bundle_init error: {e:?}");
              let _ = tx.send(Err(e));
              return Ok(());
            }
          };
        log::trace!("bundler.build");
        let output = match bundler.build().await {
          Ok(output) => output,
          Err(e) => {
            // Hard bundler errors are returned as a structured result
            // (`success: false` with `errors`), not as a thrown exception.
            match e.downcast::<super::BundleBuildFailure>() {
              Ok(failure) => {
                let _ = tx.send(Ok(super::build_failure_to_response(&failure)));
                return Ok(());
              }
              Err(e) => {
                log::trace!("bundler.build error: {e:?}");
                let _ = tx.send(Err(e));
                return Ok(());
              }
            }
          }
        };
        log::trace!("process_result");
        if write_output {
          super::process_result(
            &output,
            &bundler.cwd,
            bundle_flags.output_dir.as_ref().map(Path::new),
            bundle_flags.output_path.as_ref().map(Path::new),
            super::should_replace_require_shim(bundle_flags.platform),
            bundle_flags.minify,
            Some(&bundler.input),
          )?;
          // Convert with no output files since we already wrote them
          let mut result = convert_bundle_output(&bundler, output);
          result.output_files = None;
          let _ = tx.send(Ok(result));
        } else {
          let result = convert_bundle_output(&bundler, output);
          let _ = tx.send(Ok(result));
        }
        Ok::<_, AnyError>(())
      })
    });
    log::trace!("rx.await");
    let response = rx.await??;
    log::trace!("response: {:?}", response);
    Ok(response)
  }
}
