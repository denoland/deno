// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_graph::GraphKind;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_terminal::colors;
use rand::Rng;

use super::installer::infer_name_from_url;
use crate::args::CompileFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::http_util::HttpClientProvider;
use crate::standalone::binary::is_standalone_binary;
use crate::standalone::binary::WriteBinOptions;

pub async fn compile(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let binary_writer = factory.create_compile_binary_writer().await?;
  let http_client = factory.http_client_provider();
  let entrypoint = cli_options.resolve_main_module()?;
  let output_path = resolve_compile_executable_output_path(
    http_client,
    &compile_flags,
    cli_options.initial_cwd(),
  )
  .await?;
  let (module_roots, include_paths) = get_module_roots_and_include_paths(
    entrypoint,
    &compile_flags,
    cli_options.initial_cwd(),
  )?;

  let graph = Arc::try_unwrap(
    module_graph_creator
      .create_graph_and_maybe_check(module_roots.clone())
      .await?,
  )
  .unwrap();
  let graph = if cli_options.type_check_mode().is_true() {
    // In this case, the previous graph creation did type checking, which will
    // create a module graph with types information in it. We don't want to
    // store that in the binary so create a code only module graph from scratch.
    module_graph_creator
      .create_graph(
        GraphKind::CodeOnly,
        module_roots,
        crate::graph_util::NpmCachingStrategy::Eager,
      )
      .await?
  } else {
    graph
  };

  log::info!(
    "{} {} to {}",
    colors::green("Compile"),
    entrypoint,
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

  let file = std::fs::File::create(&temp_path).with_context(|| {
    format!("Opening temporary file '{}'", temp_path.display())
  })?;

  let write_result = binary_writer
    .write_bin(WriteBinOptions {
      writer: file,
      display_output_filename: &output_path
        .file_name()
        .unwrap()
        .to_string_lossy(),
      graph: &graph,
      entrypoint,
      include_paths: &include_paths,
      exclude_paths: compile_flags
        .exclude
        .iter()
        .map(|p| cli_options.initial_cwd().join(p))
        .chain(std::iter::once(
          cli_options.initial_cwd().join(&output_path),
        ))
        .chain(std::iter::once(cli_options.initial_cwd().join(&temp_path)))
        .collect(),
      compile_flags: &compile_flags,
    })
    .await
    .with_context(|| {
      format!(
        "Writing deno compile executable to temporary file '{}'",
        temp_path.display()
      )
    });

  // set it as executable
  #[cfg(unix)]
  let write_result = write_result.and_then(|_| {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&temp_path, perms).with_context(|| {
      format!(
        "Setting permissions on temporary file '{}'",
        temp_path.display()
      )
    })
  });

  let write_result = write_result.and_then(|_| {
    std::fs::rename(&temp_path, &output_path).with_context(|| {
      format!(
        "Renaming temporary file '{}' to '{}'",
        temp_path.display(),
        output_path.display()
      )
    })
  });

  if let Err(err) = write_result {
    // errored, so attempt to remove the temporary file
    let _ = std::fs::remove_file(temp_path);
    return Err(err);
  }

  Ok(())
}

pub async fn compile_eszip(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let module_graph_creator = factory.module_graph_creator().await?;
  let parsed_source_cache = factory.parsed_source_cache();
  let tsconfig_resolver = factory.tsconfig_resolver()?;
  let http_client = factory.http_client_provider();
  let entrypoint = cli_options.resolve_main_module()?;
  let mut output_path = resolve_compile_executable_output_path(
    http_client,
    &compile_flags,
    cli_options.initial_cwd(),
  )
  .await?;
  output_path.set_extension("eszip");

  let maybe_import_map_specifier =
    cli_options.resolve_specified_import_map_specifier()?;
  let (module_roots, _include_paths) = get_module_roots_and_include_paths(
    entrypoint,
    &compile_flags,
    cli_options.initial_cwd(),
  )?;

  let graph = Arc::try_unwrap(
    module_graph_creator
      .create_graph_and_maybe_check(module_roots.clone())
      .await?,
  )
  .unwrap();
  let graph = if cli_options.type_check_mode().is_true() {
    // In this case, the previous graph creation did type checking, which will
    // create a module graph with types information in it. We don't want to
    // store that in the binary so create a code only module graph from scratch.
    module_graph_creator
      .create_graph(
        GraphKind::CodeOnly,
        module_roots,
        crate::graph_util::NpmCachingStrategy::Eager,
      )
      .await?
  } else {
    graph
  };

  let transpile_and_emit_options = tsconfig_resolver
    .transpile_and_emit_options(cli_options.workspace().root_dir())?;
  let transpile_options = transpile_and_emit_options.transpile.clone();
  let emit_options = transpile_and_emit_options.emit.clone();

  let parser = parsed_source_cache.as_capturing_parser();
  let root_dir_url = cli_options.workspace().root_dir();
  log::debug!("Binary root dir: {}", root_dir_url);
  let relative_file_base = eszip::EszipRelativeFileBaseUrl::new(root_dir_url);
  let mut eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
    graph,
    parser,
    transpile_options,
    emit_options,
    relative_file_base: Some(relative_file_base),
    npm_packages: None,
    module_kind_resolver: Default::default(),
  })?;

  if let Some(import_map_specifier) = maybe_import_map_specifier {
    let import_map_path = import_map_specifier.to_file_path().unwrap();
    let import_map_content = std::fs::read_to_string(&import_map_path)
      .with_context(|| {
        format!("Failed to read import map: {:?}", import_map_path)
      })?;

    let import_map_specifier_str = if let Some(relative_import_map_specifier) =
      root_dir_url.make_relative(&import_map_specifier)
    {
      relative_import_map_specifier
    } else {
      import_map_specifier.to_string()
    };

    eszip.add_import_map(
      eszip::ModuleKind::Json,
      import_map_specifier_str,
      import_map_content.as_bytes().to_vec().into(),
    );
  }

  log::info!(
    "{} {} to {}",
    colors::green("Compile"),
    entrypoint,
    output_path.display(),
  );
  validate_output_path(&output_path)?;

  let mut file = std::fs::File::create(&output_path).with_context(|| {
    format!("Opening ESZip file '{}'", output_path.display())
  })?;

  let write_result = {
    let r = file.write_all(&eszip.into_bytes());
    drop(file);
    r
  };

  if let Err(err) = write_result {
    let _ = std::fs::remove_file(output_path);
    return Err(err.into());
  }

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
    if !is_standalone_binary(output_path) {
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

fn get_module_roots_and_include_paths(
  entrypoint: &ModuleSpecifier,
  compile_flags: &CompileFlags,
  initial_cwd: &Path,
) -> Result<(Vec<ModuleSpecifier>, Vec<ModuleSpecifier>), AnyError> {
  fn is_module_graph_module(url: &ModuleSpecifier) -> bool {
    if url.scheme() != "file" {
      return true;
    }
    is_module_graph_media_type(MediaType::from_specifier(url))
  }

  fn is_module_graph_media_type(media_type: MediaType) -> bool {
    match media_type {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx
      | MediaType::Json
      | MediaType::Wasm => true,
      MediaType::Css
      | MediaType::Html
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => false,
    }
  }

  fn analyze_path(
    url: &ModuleSpecifier,
    excluded_paths: &HashSet<PathBuf>,
    searched_paths: &mut HashSet<PathBuf>,
    mut add_path: impl FnMut(&Path),
  ) -> Result<(), AnyError> {
    let Ok(path) = url_to_file_path(url) else {
      return Ok(());
    };
    let mut pending = VecDeque::from([path]);
    while let Some(path) = pending.pop_front() {
      if !searched_paths.insert(path.clone()) {
        continue;
      }
      if excluded_paths.contains(&path) {
        continue;
      }
      if !path.is_dir() {
        add_path(&path);
        continue;
      }
      for entry in std::fs::read_dir(&path).with_context(|| {
        format!("Failed reading directory '{}'", path.display())
      })? {
        let entry = entry.with_context(|| {
          format!("Failed reading entry in directory '{}'", path.display())
        })?;
        pending.push_back(entry.path());
      }
    }
    Ok(())
  }

  let mut searched_paths = HashSet::new();
  let mut module_roots = Vec::new();
  let mut include_paths = Vec::new();
  let exclude_set = compile_flags
    .exclude
    .iter()
    .map(|path| initial_cwd.join(path))
    .collect::<HashSet<_>>();
  module_roots.push(entrypoint.clone());
  for side_module in &compile_flags.include {
    let url = resolve_url_or_path(side_module, initial_cwd)?;
    if is_module_graph_module(&url) {
      module_roots.push(url.clone());
    } else {
      analyze_path(&url, &exclude_set, &mut searched_paths, |file_path| {
        let media_type = MediaType::from_path(file_path);
        if is_module_graph_media_type(media_type) {
          if let Ok(file_url) = url_from_file_path(file_path) {
            module_roots.push(file_url);
          }
        }
      })?;
    }
    if url.scheme() == "file" {
      include_paths.push(url);
    }
  }
  Ok((module_roots, include_paths))
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

  output_path.ok_or_else(|| anyhow!(
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
        icon: None,
        include: Default::default(),
        exclude: Default::default(),
        eszip: true,
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
        include: Default::default(),
        exclude: Default::default(),
        icon: None,
        no_terminal: false,
        eszip: true,
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
