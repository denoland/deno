// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CompileFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::http_util::HttpClientProvider;
use crate::standalone::is_standalone_binary;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_graph::GraphKind;
use deno_terminal::colors;
use eszip::EszipV2;
use rand::Rng;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::installer::infer_name_from_url;

pub async fn compile(
  flags: Flags,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags)?;
  let cli_options = factory.cli_options();
  let module_graph_creator = factory.module_graph_creator().await?;
  let parsed_source_cache = factory.parsed_source_cache();
  let binary_writer = factory.create_compile_binary_writer().await?;
  let http_client = factory.http_client_provider();
  let module_specifier = cli_options.resolve_main_module()?;
  let module_roots = {
    let mut vec = Vec::with_capacity(compile_flags.include.len() + 1);
    vec.push(module_specifier.clone());
    for side_module in &compile_flags.include {
      vec.push(resolve_url_or_path(side_module, cli_options.initial_cwd())?);
    }
    vec
  };

  // this is not supported, so show a warning about it, but don't error in order
  // to allow someone to still run `deno compile` when this is in a deno.json
  if cli_options.unstable_sloppy_imports() {
    log::warn!(
      concat!(
        "{} Sloppy imports are not supported in deno compile. ",
        "The compiled executable may encouter runtime errors.",
      ),
      crate::colors::yellow("Warning"),
    );
  }

  let output_path = resolve_compile_executable_output_path(
    http_client,
    &compile_flags,
    cli_options.initial_cwd(),
  )
  .await?;

  let graph = Arc::try_unwrap(
    module_graph_creator
      .create_graph_and_maybe_check(module_roots.clone())
      .await?,
  )
  .unwrap();
  let graph = if cli_options.type_check_mode().is_true() {
    // In this case, the previous graph creation did type checking, which will
    // create a module graph with types information in it. We don't want to
    // store that in the eszip so create a code only module graph from scratch.
    module_graph_creator
      .create_graph(GraphKind::CodeOnly, module_roots)
      .await?
  } else {
    graph
  };

  let ts_config_for_emit =
    cli_options.resolve_ts_config_for_emit(deno_config::TsConfigType::Emit)?;
  let (transpile_options, emit_options) =
    crate::args::ts_config_to_transpile_and_emit_options(
      ts_config_for_emit.ts_config,
    )?;
  let parser = parsed_source_cache.as_capturing_parser();
  let mut eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
    graph,
    parser,
    transpile_options,
    emit_options,
    relative_file_base: None,
  })?;

  log::info!(
    "{} {} to {}",
    colors::green("Compile"),
    module_specifier.to_string(),
    output_path.display(),
  );
  validate_output_path(&output_path)?;

  let mut temp_filename = output_path.file_name().unwrap().to_owned();
  temp_filename.push(format!(
    ".tmp-{}",
    faster_hex::hex_encode(
      &rand::thread_rng().gen::<[u8; 8]>(),
      &mut [0u8; 16]
    )
    .unwrap()
  ));
  let temp_path = output_path.with_file_name(temp_filename);

  let mut file = std::fs::File::create(&temp_path).with_context(|| {
    format!("Opening temporary file '{}'", temp_path.display())
  })?;
  let write_result = if compile_flags.eszip {
    // TODO: write npm vfs
    let (_, _, node_modules) = binary_writer.pack_node_modules(&mut eszip)?;
    if let Some(node_modules) = node_modules {
      eszip.add_opaque_data(
        "internal://node_modules".to_string(),
        Arc::from(deno_core::serde_json::to_string(&node_modules)?.as_bytes()),
      );
    }
    file.write_all(&eszip.into_bytes()).map_err(AnyError::from)
  } else {
    binary_writer
      .write_bin(
        &mut file,
        eszip,
        &module_specifier,
        &compile_flags,
        cli_options,
      )
      .await
  }
  .with_context(|| format!("Writing temporary file '{}'", temp_path.display()));
  drop(file);

  // set it as executable
  #[cfg(unix)]
  let write_result = if write_result.is_ok() && !compile_flags.eszip {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&temp_path, perms).with_context(|| {
      format!(
        "Setting permissions on temporary file '{}'",
        temp_path.display()
      )
    })
  } else {
    write_result
  };

  if let Err(err) = write_result {
    // errored, so attempt to remove the output path
    let _ = std::fs::remove_file(temp_path);
    return Err(err);
  }

  std::fs::rename(&temp_path, &output_path).with_context(|| {
    format!(
      "Renaming temporary file '{}' to '{}'",
      temp_path.display(),
      output_path.display()
    )
  })?;

  Ok(())
}

/// This function writes out a final binary to specified path. If output path
/// is not already standalone binary it will return error instead.
fn validate_output_path(output_path: &Path) -> Result<(), AnyError> {
  if output_path.exists() {
    // If the output is a directory, throw error
    if output_path.is_dir() {
      bail!(
        concat!(
          "Could not compile to file '{}' because a directory exists with ",
          "the same name. You can use the `--output <file-path>` flag to ",
          "provide an alternative name."
        ),
        output_path.display()
      );
    }

    // Make sure we don't overwrite any file not created by Deno compiler because
    // this filename is chosen automatically in some cases.
    if !is_standalone_binary(output_path) && !is_eszip(output_path) {
      bail!(
        concat!(
          "Could not compile to file '{}' because the file already exists ",
          "and cannot be overwritten. Please delete the existing file or ",
          "use the `--output <file-path>` flag to provide an alternative name."
        ),
        output_path.display()
      );
    }

    // Remove file if it was indeed a deno compiled binary, to avoid corruption
    // (see https://github.com/denoland/deno/issues/10310)
    std::fs::remove_file(output_path)?;
  } else {
    let output_base = &output_path.parent().unwrap();
    if output_base.exists() && output_base.is_file() {
      bail!(
          concat!(
            "Could not compile to file '{}' because its parent directory ",
            "is an existing file. You can use the `--output <file-path>` flag to ",
            "provide an alternative name.",
          ),
          output_base.display(),
        );
    }
    std::fs::create_dir_all(output_base)?;
  }

  Ok(())
}

async fn resolve_compile_executable_output_path(
  http_client_provider: &HttpClientProvider,
  compile_flags: &CompileFlags,
  current_dir: &Path,
) -> Result<PathBuf, AnyError> {
  let module_specifier =
    resolve_url_or_path(&compile_flags.source_file, current_dir)?;

  let output_flag = compile_flags.output.clone();
  let mut output_path = if let Some(out) = output_flag.as_ref() {
    let mut out_path = PathBuf::from(out);
    if out.ends_with('/') || out.ends_with('\\') {
      if let Some(infer_file_name) =
        infer_name_from_url(http_client_provider, &module_specifier)
          .await
          .map(PathBuf::from)
      {
        out_path = out_path.join(infer_file_name);
      }
    } else {
      out_path = out_path.to_path_buf();
    }
    Some(out_path)
  } else {
    None
  };

  if output_flag.is_none() {
    output_path = infer_name_from_url(http_client_provider, &module_specifier)
      .await
      .map(PathBuf::from)
  }

  output_path.ok_or_else(|| generic_error(
    "An executable name was not provided. One could not be inferred from the URL. Aborting.",
  )).map(|output_path| {
    get_os_specific_filepath(output_path, &compile_flags.target)
  })
}

fn get_os_specific_filepath(
  output: PathBuf,
  target: &Option<String>,
) -> PathBuf {
  let is_windows = match target {
    Some(target) => target.contains("windows"),
    None => cfg!(windows),
  };
  if is_windows && output.extension().unwrap_or_default() != "exe" {
    if let Some(ext) = output.extension() {
      // keep version in my-exe-0.1.0 -> my-exe-0.1.0.exe
      output.with_extension(format!("{}.exe", ext.to_string_lossy()))
    } else {
      output.with_extension("exe")
    }
  } else {
    output
  }
}

pub fn is_eszip(path: &Path) -> bool {
  let Ok(mut file) = std::fs::File::open(path) else {
    return false;
  };
  let mut magic = [0u8; 8];
  if file.read_exact(&mut magic).is_err() {
    return false;
  }
  EszipV2::has_magic(&magic)
}

#[cfg(test)]
mod test {
  pub use super::*;

  #[tokio::test]
  async fn resolve_compile_executable_output_path_target_linux() {
    let http_client = HttpClientProvider::new(None, None);
    let path = resolve_compile_executable_output_path(
      &http_client,
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: Some(String::from("./file")),
        args: Vec::new(),
        target: Some("x86_64-unknown-linux-gnu".to_string()),
        no_terminal: false,
        include: vec![],
        eszip: false,
      },
      &std::env::current_dir().unwrap(),
    )
    .await
    .unwrap();

    // no extension, no matter what the operating system is
    // because the target was specified as linux
    // https://github.com/denoland/deno/issues/9667
    assert_eq!(path.file_name().unwrap(), "file");
  }

  #[tokio::test]
  async fn resolve_compile_executable_output_path_target_windows() {
    let http_client = HttpClientProvider::new(None, None);
    let path = resolve_compile_executable_output_path(
      &http_client,
      &CompileFlags {
        source_file: "mod.ts".to_string(),
        output: Some(String::from("./file")),
        args: Vec::new(),
        target: Some("x86_64-pc-windows-msvc".to_string()),
        include: vec![],
        no_terminal: false,
        eszip: false,
      },
      &std::env::current_dir().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "file.exe");
  }

  #[test]
  fn test_os_specific_file_path() {
    fn run_test(path: &str, target: Option<&str>, expected: &str) {
      assert_eq!(
        get_os_specific_filepath(
          PathBuf::from(path),
          &target.map(|s| s.to_string())
        ),
        PathBuf::from(expected)
      );
    }

    if cfg!(windows) {
      run_test("C:\\my-exe", None, "C:\\my-exe.exe");
      run_test("C:\\my-exe.exe", None, "C:\\my-exe.exe");
      run_test("C:\\my-exe-0.1.2", None, "C:\\my-exe-0.1.2.exe");
    } else {
      run_test("my-exe", Some("linux"), "my-exe");
      run_test("my-exe-0.1.2", Some("linux"), "my-exe-0.1.2");
    }

    run_test("C:\\my-exe", Some("windows"), "C:\\my-exe.exe");
    run_test("C:\\my-exe.exe", Some("windows"), "C:\\my-exe.exe");
    run_test("C:\\my-exe.0.1.2", Some("windows"), "C:\\my-exe.0.1.2.exe");
    run_test("my-exe-0.1.2", Some("linux"), "my-exe-0.1.2");
  }
}
