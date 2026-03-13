// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::SourceMapOption;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileOptions;
use deno_ast::TranspileResult;
use deno_core::anyhow;
use deno_core::error::AnyError;
use deno_terminal::colors;

use crate::args::Flags;
use crate::args::SourceMapMode;
use crate::args::TranspileFlags;
use crate::util::display;
use crate::util::fs::canonicalize_path;

pub fn transpile(
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

  let cwd = crate::util::env::resolve_cwd(flags.initial_cwd.as_deref())?;

  let source_map_option = match transpile_flags.source_map {
    SourceMapMode::None => SourceMapOption::None,
    SourceMapMode::Inline => SourceMapOption::Inline,
    SourceMapMode::Separate => SourceMapOption::Separate,
  };

  for file_path_str in files {
    let file_path = cwd.join(file_path_str);
    let file_path = canonicalize_path(&file_path).unwrap_or(file_path.clone());

    let media_type = MediaType::from_path(&file_path);
    if !media_type.is_emittable() {
      log::warn!(
        "{} Skipping {} (not a TypeScript/JSX/TSX file)",
        colors::yellow("Warning"),
        file_path.display()
      );
      continue;
    }

    let source_code = std::fs::read_to_string(&file_path)?;
    let specifier = deno_core::ModuleSpecifier::from_file_path(&file_path)
      .map_err(|_| {
        anyhow::anyhow!("Invalid file path: {}", file_path.display())
      })?;

    let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: source_code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;

    let transpile_result = parsed_source.transpile(
      &TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &TranspileModuleOptions { module_kind: None },
      &EmitOptions {
        source_map: source_map_option,
        source_map_file: Some(specifier.to_string()),
        ..Default::default()
      },
    )?;

    let emitted = match transpile_result {
      TranspileResult::Owned(source) => source,
      TranspileResult::Cloned(source) => source,
    };

    let js_text = &emitted.text;
    let source_map_text = &emitted.source_map;

    // Determine output path
    if files.len() == 1
      && transpile_flags.output.is_none()
      && transpile_flags.output_dir.is_none()
    {
      // Single file, no output specified: print to stdout
      display::write_to_stdout_ignore_sigpipe(js_text.as_bytes())?;
      if let Some(sm) = source_map_text
        && matches!(transpile_flags.source_map, SourceMapMode::Separate)
      {
        log::warn!("// source map:");
        display::write_to_stdout_ignore_sigpipe(sm.as_bytes())?;
      }
    } else {
      let output_path = compute_output_path(
        &file_path,
        &cwd,
        transpile_flags.output.as_deref(),
        transpile_flags.output_dir.as_deref(),
      )?;

      // Ensure parent directory exists
      if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
      }

      std::fs::write(&output_path, js_text)?;
      log::info!("{} {}", colors::green("Emit"), output_path.display());

      // Write separate source map file
      if let Some(sm) = source_map_text
        && matches!(transpile_flags.source_map, SourceMapMode::Separate)
      {
        let map_path = output_path.with_extension("js.map");
        std::fs::write(&map_path, sm)?;
        log::info!("{} {}", colors::green("Emit"), map_path.display());
      }
    }
  }

  Ok(())
}

fn compute_output_path(
  input_path: &Path,
  cwd: &Path,
  output: Option<&str>,
  output_dir: Option<&str>,
) -> Result<PathBuf, AnyError> {
  if let Some(output) = output {
    // Explicit output file
    return Ok(cwd.join(output));
  }

  // Change extension to .js
  let js_filename = input_path.with_extension("js");

  if let Some(outdir) = output_dir {
    let outdir = cwd.join(outdir);
    // Try to maintain relative structure
    let file_name = js_filename
      .file_name()
      .ok_or_else(|| anyhow::anyhow!("Invalid file path"))?;
    Ok(outdir.join(file_name))
  } else {
    // Write alongside source file
    Ok(js_filename)
  }
}
