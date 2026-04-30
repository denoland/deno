// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::SourceMapOption;
use deno_ast::TranspileModuleOptions;
use deno_config::deno_json::CompilerOptions;
use deno_core::ModuleSpecifier;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_terminal::colors;
use sys_traits::PathsInErrorsExt;

use crate::args::Flags;
use crate::args::SourceMapMode;
use crate::args::TranspileFlags;
use crate::args::TypeCheckMode;
use crate::factory::CliFactory;
use crate::tsc;
use crate::util::display;

pub async fn transpile(
  flags: Arc<Flags>,
  transpile_flags: TranspileFlags,
) -> Result<(), AnyError> {
  let files = &transpile_flags.files;

  if files.is_empty() {
    anyhow::bail!("No input files specified");
  }

  if files.len() > 1 && transpile_flags.output.is_some() {
    anyhow::bail!(
      "Cannot use --output with multiple input files. Use --outdir instead."
    );
  }

  if transpile_flags.declaration
    && transpile_flags.output.is_none()
    && transpile_flags.output_dir.is_none()
  {
    anyhow::bail!(
      "Cannot use --declaration without --output or --outdir. Declaration files must be written to disk."
    );
  }

  let is_stdout_mode = files.len() == 1
    && transpile_flags.output.is_none()
    && transpile_flags.output_dir.is_none();

  if is_stdout_mode
    && matches!(transpile_flags.source_map, SourceMapMode::Separate)
  {
    anyhow::bail!(
      "Cannot use --source-map separate when outputting to stdout. Use --output or --outdir, or use --source-map inline instead."
    );
  }

  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let cwd = cli_options.initial_cwd();
  let real_sys = factory.sys();
  let sys = real_sys.with_paths_in_errors();

  let source_map_option = match transpile_flags.source_map {
    SourceMapMode::None => SourceMapOption::None,
    SourceMapMode::Inline => SourceMapOption::Inline,
    SourceMapMode::Separate => SourceMapOption::Separate,
  };

  // Resolve transpile options from deno.json compilerOptions
  let compiler_options_resolver = factory.compiler_options_resolver()?;

  // Collect file paths and specifiers
  let mut file_entries = Vec::new();
  for file_path_str in files {
    let file_path = cwd.join(file_path_str);

    let media_type = MediaType::from_path(&file_path);
    if !media_type.is_emittable() {
      log::warn!(
        "{} Skipping {} (not a TypeScript/JSX/TSX file)",
        colors::yellow("Warning"),
        file_path.display()
      );
      continue;
    }

    let specifier =
      ModuleSpecifier::from_file_path(&file_path).map_err(|_| {
        anyhow::anyhow!("Invalid file path: {}", file_path.display())
      })?;

    file_entries.push((file_path, specifier, media_type));
  }

  if file_entries.is_empty() {
    anyhow::bail!(
      "No emittable files found. Only TypeScript/JSX/TSX files can be transpiled."
    );
  }

  // Transpile each file (TS -> JS)
  for (file_path, specifier, media_type) in &file_entries {
    let source_bytes = sys
      .fs_read(file_path)
      .with_context(|| format!("Failed to read {}", file_path.display()))?;
    let source_code = String::from_utf8(source_bytes.into_owned())
      .with_context(|| format!("{} is not valid UTF-8", file_path.display()))?;

    let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: source_code.into(),
      media_type: *media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;

    // Use transpile options from deno.json, with source map overridden by CLI flag
    let transpile_and_emit_options = compiler_options_resolver
      .for_specifier(specifier)
      .transpile_options()?;
    let transpile_options = &transpile_and_emit_options.transpile;

    // Compute the output path/filename so source_map_file references the
    // generated file (e.g. "out.js"), not the input (e.g. "main.ts").
    let maybe_output_path = if !is_stdout_mode {
      Some(compute_output_path(
        file_path,
        cwd,
        transpile_flags.output.as_deref(),
        transpile_flags.output_dir.as_deref(),
        *media_type,
      )?)
    } else {
      None
    };
    let source_map_filename = if let Some(output_path) = &maybe_output_path {
      output_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
    } else {
      // In stdout mode, use the input-derived JS filename
      let ext = js_extension_for_media_type(*media_type);
      file_path
        .with_extension(ext)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
    };

    // Always apply the CLI source map mode. The built-in emit defaults
    // have inlineSourceMap: true (for the runtime), but `deno transpile`
    // should produce clean output by default. Users must pass --source-map
    // explicitly to enable source maps.
    let emit_options = EmitOptions {
      source_map: source_map_option,
      source_map_file: Some(source_map_filename),
      ..transpile_and_emit_options.emit.clone()
    };

    let transpile_result = parsed_source.transpile(
      transpile_options,
      &TranspileModuleOptions { module_kind: None },
      &emit_options,
    )?;

    let emitted = transpile_result.into_source();
    let js_text = &emitted.text;
    let source_map_text = &emitted.source_map;

    // Determine output path
    if is_stdout_mode {
      // Single file, no output specified: print to stdout
      display::write_to_stdout_ignore_sigpipe(js_text.as_bytes())?;
    } else {
      let output_path = maybe_output_path.unwrap();

      // Ensure parent directory exists
      if let Some(parent) = output_path.parent() {
        sys.fs_create_dir_all(parent).with_context(|| {
          format!("Failed to create directory {}", parent.display())
        })?;
      }

      sys
        .fs_write(&output_path, js_text.as_bytes())
        .with_context(|| {
          format!("Failed to write {}", output_path.display())
        })?;
      log::info!("{} {}", colors::green("Emit"), output_path.display());

      // Write separate source map file
      if let Some(sm) = source_map_text
        && matches!(transpile_flags.source_map, SourceMapMode::Separate)
      {
        let ext = output_path
          .extension()
          .unwrap_or_default()
          .to_string_lossy();
        let map_path = output_path.with_extension(format!("{ext}.map"));
        sys
          .fs_write(&map_path, sm.as_bytes())
          .with_context(|| format!("Failed to write {}", map_path.display()))?;
        log::info!("{} {}", colors::green("Emit"), map_path.display());
      }
    }
  }

  // Generate .d.ts declaration files if requested
  if transpile_flags.declaration && !file_entries.is_empty() {
    emit_declarations(&factory, &transpile_flags, &file_entries, cwd).await?;
  }

  Ok(())
}

async fn emit_declarations(
  factory: &CliFactory,
  transpile_flags: &TranspileFlags,
  file_entries: &[(PathBuf, ModuleSpecifier, MediaType)],
  cwd: &Path,
) -> Result<(), AnyError> {
  let cli_options = factory.cli_options()?;
  let real_sys = factory.sys();
  let sys = real_sys.with_paths_in_errors();

  // Build a module graph for the input files
  let root_names: Vec<(ModuleSpecifier, MediaType)> = file_entries
    .iter()
    .map(|(_, specifier, media_type)| (specifier.clone(), *media_type))
    .collect();

  let specifiers: Vec<ModuleSpecifier> =
    root_names.iter().map(|(s, _)| s.clone()).collect();

  let mut graph = ModuleGraph::new(GraphKind::All);
  let module_graph_builder = factory.module_graph_builder().await?;
  module_graph_builder
    .build_graph_roots_with_npm_resolution(
      &mut graph,
      specifiers,
      crate::graph_util::BuildGraphWithNpmOptions {
        is_dynamic: false,
        loader: None,
        npm_caching: cli_options.default_npm_caching_strategy(),
      },
    )
    .await?;

  // Resolve compiler options, adding declaration-specific settings.
  // Since TSC type-checks all files in a single pass, all files must share
  // the same compiler options scope. Error if they span different scopes.
  let compiler_options_resolver = factory.compiler_options_resolver()?;
  let first_specifier = &root_names[0].0;
  let (first_key, first_data) =
    compiler_options_resolver.entry_for_specifier(first_specifier);
  for (specifier, _) in root_names.iter().skip(1) {
    let (key, _) = compiler_options_resolver.entry_for_specifier(specifier);
    if key != first_key {
      anyhow::bail!(
        "Cannot generate declarations for files with different compiler option scopes.\n\
         '{}' uses scope '{}' but '{}' uses scope '{}'.\n\
         Separate these files into different invocations.",
        first_specifier,
        first_key,
        specifier,
        key,
      );
    }
  }
  let base_compiler_options =
    first_data.compiler_options_for_lib(cli_options.ts_type_lib_window())?;

  // Merge declaration options into the base compiler options
  let mut config_value =
    deno_core::serde_json::to_value(base_compiler_options.as_ref())?;
  let config_obj = config_value
    .as_object_mut()
    .ok_or_else(|| anyhow::anyhow!("Invalid compiler options"))?;
  config_obj.insert("declaration".into(), json!(true));
  config_obj.insert("emitDeclarationOnly".into(), json!(true));
  config_obj.insert("noEmit".into(), json!(false));

  let compiler_options = Arc::new(CompilerOptions::new(config_value));

  let hash_data =
    deno_lib::util::hash::FastInsecureHasher::new_deno_versioned()
      .write_hashable(&compiler_options)
      .finish();

  let jsx_import_source_config_resolver = Arc::new(
    deno_resolver::deno_json::JsxImportSourceConfigResolver::from_compiler_options_resolver(
      compiler_options_resolver,
    )?,
  );

  // Set up npm state
  let type_checker = factory.type_checker().await?;
  let maybe_npm = Some(type_checker.create_request_npm_state());

  // Note: We use tsc::exec directly because TypeChecker.check_diagnostics
  // does not support returning emitted declaration files.
  let response = tsc::exec(
    tsc::Request {
      config: compiler_options,
      debug: cli_options.log_level() == Some(log::Level::Debug),
      graph: Arc::new(graph),
      jsx_import_source_config_resolver,
      hash_data,
      maybe_npm,
      maybe_tsbuildinfo: None,
      root_names,
      // Declaration emit requires full type-checking regardless of
      // the user's type_check_mode setting.
      check_mode: TypeCheckMode::All,
      initial_cwd: cwd.to_path_buf(),
      capture_emitted_files: true,
    },
    None,
  )?;

  if response.diagnostics.has_diagnostic() {
    anyhow::bail!("Type checking failed:\n{}", response.diagnostics);
  }

  // Determine the output directory for .d.ts files.
  // With -o, place .d.ts next to the output file.
  // With --outdir, use the outdir.
  // Otherwise, place next to the source file (handled by TSC path).
  let output_base_dir = if let Some(output) = &transpile_flags.output {
    let output_path = cwd.join(output);
    output_path.parent().map(|p| p.to_path_buf())
  } else {
    None
  };

  // Write emitted .d.ts files
  for (file_name, content) in &response.emitted_files {
    // The file names from TSC are specifier-based, convert to output paths
    let output_path = resolve_dts_output_path(
      file_name,
      transpile_flags.output_dir.as_deref(),
      output_base_dir.as_deref(),
      cwd,
    )?;

    if let Some(parent) = output_path.parent() {
      sys.fs_create_dir_all(parent).with_context(|| {
        format!("Failed to create directory {}", parent.display())
      })?;
    }

    sys
      .fs_write(&output_path, content.as_bytes())
      .with_context(|| format!("Failed to write {}", output_path.display()))?;
    log::info!("{} {}", colors::green("Emit"), output_path.display());
  }

  if response.emitted_files.is_empty() {
    log::warn!(
      "{} No declaration files were emitted",
      colors::yellow("Warning")
    );
  }

  Ok(())
}

fn resolve_dts_output_path(
  tsc_file_name: &str,
  output_dir: Option<&str>,
  output_base_dir: Option<&Path>,
  cwd: &Path,
) -> Result<PathBuf, AnyError> {
  // TSC emits file names like "file:///path/to/file.d.ts"
  let path = if let Ok(specifier) = ModuleSpecifier::parse(tsc_file_name) {
    deno_path_util::url_to_file_path(&specifier).with_context(|| {
      format!("Cannot convert specifier to path: {tsc_file_name}")
    })?
  } else {
    PathBuf::from(tsc_file_name)
  };

  if let Some(outdir) = output_dir {
    // --outdir: preserve relative directory structure
    let outdir = cwd.join(outdir);
    let relative = path.strip_prefix(cwd).map_err(|_| {
      anyhow::anyhow!(
        "Declaration file {} is not under the current directory",
        path.display()
      )
    })?;
    Ok(outdir.join(relative))
  } else if let Some(base_dir) = output_base_dir {
    // -o: place .d.ts next to the output file
    let file_name = path
      .file_name()
      .ok_or_else(|| anyhow::anyhow!("Invalid declaration file path"))?;
    Ok(base_dir.join(file_name))
  } else {
    // No output specified: place next to source file
    Ok(path)
  }
}

fn js_extension_for_media_type(media_type: MediaType) -> &'static str {
  match media_type {
    MediaType::Mts => "mjs",
    MediaType::Cts => "cjs",
    _ => "js",
  }
}

fn compute_output_path(
  input_path: &Path,
  cwd: &Path,
  output: Option<&str>,
  output_dir: Option<&str>,
  media_type: MediaType,
) -> Result<PathBuf, AnyError> {
  if let Some(output) = output {
    // Explicit output file
    return Ok(cwd.join(output));
  }

  let ext = js_extension_for_media_type(media_type);
  let js_filename = input_path.with_extension(ext);

  if let Some(outdir) = output_dir {
    let outdir = cwd.join(outdir);
    let relative = js_filename
      .strip_prefix(cwd)
      .map_err(|_| {
        anyhow::anyhow!(
          "Input file {} is not under the current directory. Use --output instead.",
          input_path.display()
        )
      })?;
    Ok(outdir.join(relative))
  } else {
    // Write alongside source file
    Ok(js_filename)
  }
}
