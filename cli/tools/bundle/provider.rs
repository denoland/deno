// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_bundle_runtime as rt_bundle;
use deno_bundle_runtime::BundleOptions as RtBundleOptions;
use deno_bundle_runtime::Plugins as RtPlugins;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;

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
      format: value.format,
      minify: value.minify,
      code_splitting: value.code_splitting,
      platform: value.platform,
      watch: false,
      sourcemap: value.sourcemap,
      inline_imports: value.inline_imports,
      packages: value.packages,
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

fn process_output_files(
  bundle_flags: &crate::args::BundleFlags,
  response: &mut esbuild_client::protocol::BuildResponse,
) -> Result<(), AnyError> {
  if let Some(files) = &mut response.output_files {
    for file in files {
      let processed_contents = crate::tools::bundle::maybe_process_contents(
        file,
        crate::tools::bundle::should_replace_require_shim(
          bundle_flags.platform,
        ),
        bundle_flags.minify,
      )?;
      if let Some(contents) = processed_contents.contents {
        file.contents = contents;
      }
    }
  }
  Ok(())
}

pub struct CliBundleResolver {
  plugin_handler: Arc<super::DenoPluginHandler>,
}

impl CliBundleResolver {
  pub fn new(plugin_handler: Arc<super::DenoPluginHandler>) -> Self {
    Self { plugin_handler }
  }
}

fn convert_import_kind(
  kind: deno_bundle_runtime::ImportKind,
) -> esbuild_client::protocol::ImportKind {
  match kind {
    deno_bundle_runtime::ImportKind::ImportStatement => {
      esbuild_client::protocol::ImportKind::ImportStatement
    }
    deno_bundle_runtime::ImportKind::ImportRule => {
      esbuild_client::protocol::ImportKind::ImportRule
    }
    deno_bundle_runtime::ImportKind::RequireCall => {
      esbuild_client::protocol::ImportKind::RequireCall
    }
    deno_bundle_runtime::ImportKind::DynamicImport => {
      esbuild_client::protocol::ImportKind::DynamicImport
    }
    deno_bundle_runtime::ImportKind::RequireResolve => {
      esbuild_client::protocol::ImportKind::RequireResolve
    }
    deno_bundle_runtime::ImportKind::ComposesFrom => {
      esbuild_client::protocol::ImportKind::ComposesFrom
    }
    deno_bundle_runtime::ImportKind::UrlToken => {
      esbuild_client::protocol::ImportKind::UrlToken
    }
    deno_bundle_runtime::ImportKind::EntryPoint => {
      esbuild_client::protocol::ImportKind::EntryPoint
    }
  }
}
fn convert_partial_message_to_message(
  message: esbuild_client::protocol::PartialMessage,
) -> rt_bundle::Message {
  rt_bundle::Message {
    text: message.text,
    location: message.location.map(convert_location),
    notes: message.notes.into_iter().map(convert_note).collect(),
  }
}
#[async_trait::async_trait]
impl deno_bundle_runtime::BundleResolver for CliBundleResolver {
  async fn resolve(
    &self,
    path: &str,
    options: deno_bundle_runtime::ResolveOptions,
  ) -> Result<deno_bundle_runtime::ResolveResult, AnyError> {
    let on_resolve_result = self
      .plugin_handler
      .on_resolve_inner(esbuild_client::OnResolveArgs {
        with: options.with.unwrap_or_default(),
        ids: vec![],
        key: 0,
        importer: options.importer,
        kind: options
          .kind
          .map(convert_import_kind)
          .unwrap_or(esbuild_client::protocol::ImportKind::ImportStatement),
        namespace: options.namespace,
        path: path.to_string(),
        resolve_dir: options.resolve_dir,
      })
      .await;
    match on_resolve_result {
      Ok(Some(on_resolve_result)) => Ok(deno_bundle_runtime::ResolveResult {
        errors: on_resolve_result
          .errors
          .map(|errors| {
            errors
              .into_iter()
              .map(convert_partial_message_to_message)
              .collect()
          })
          .unwrap_or_default(),
        warnings: on_resolve_result
          .warnings
          .map(|warnings| {
            warnings
              .into_iter()
              .map(convert_partial_message_to_message)
              .collect()
          })
          .unwrap_or_default(),
        external: on_resolve_result.external.unwrap_or(false),
        namespace: on_resolve_result.namespace.unwrap_or_default(),
        path: on_resolve_result.path.unwrap_or_default(),
        side_effects: on_resolve_result.side_effects.unwrap_or(false),
        suffix: on_resolve_result.suffix.unwrap_or_default(),
        plugin_data: on_resolve_result.plugin_data.unwrap_or(0),
      }),
      Ok(None) => Ok(deno_bundle_runtime::ResolveResult {
        errors: vec![deno_bundle_runtime::Message {
          text: "Could not resolve module".to_string(),
          ..Default::default()
        }],
        ..Default::default()
      }),
      Err(e) => Err(AnyError::from(e)),
    }
  }
}

#[async_trait::async_trait]
impl deno_bundle_runtime::BundleProvider for CliBundleProvider {
  async fn bundle(
    &self,
    options: RtBundleOptions,
    plugins: Option<RtPlugins>,
  ) -> Result<rt_bundle::BundleFuture, AnyError> {
    let mut flags_clone = (*self.flags).clone();
    flags_clone.type_check_mode = crate::args::TypeCheckMode::None;
    let write_output = options.write
      && (options.output_dir.is_some() || options.output_path.is_some());
    let bundle_flags: crate::args::BundleFlags = options.into();
    flags_clone.subcommand = DenoSubcommand::Bundle(bundle_flags.clone());
    let (tx, rx) = tokio::sync::oneshot::channel();
    let (resolver_tx, resolver_rx) = tokio::sync::oneshot::channel();
    let flags = Arc::new(flags_clone);

    std::thread::spawn(move || {
      deno_runtime::tokio_util::create_and_run_current_thread(async move {
        let bundler =
          match super::bundle_init(flags, &bundle_flags, plugins).await {
            Ok(bundler) => bundler,
            Err(e) => {
              log::trace!("bundle_init error: {e:?}");
              let _ = resolver_tx.send(Err(e));
              return Ok(());
            }
          };
        let plugin_handler = bundler.plugin_handler.clone();
        let resolver = Arc::new(CliBundleResolver::new(plugin_handler.clone()));
        let _ = resolver_tx.send(Ok(resolver));
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
        } else {
          process_output_files(&bundle_flags, &mut result)?;
        }
        log::trace!("convert_build_response");
        let result = convert_build_response(result);
        log::trace!("send result");
        let _ = tx.send(Ok(result));
        Ok::<_, AnyError>(())
      })
    });
    let resolver = resolver_rx.await??;
    let future = async move {
      log::trace!("rx.await");
      let response = rx.await??;

      log::trace!("response: {:?}", response);
      Ok(response)
    }
    .boxed();
    Ok(rt_bundle::BundleFuture {
      response: future,
      resolver,
    })
  }
}
