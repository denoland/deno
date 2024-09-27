// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::check_warn_tsconfig;
use crate::args::CompileFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::http_util::HttpClientProvider;
use crate::standalone::is_standalone_binary;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_graph::GraphKind;
use deno_terminal::colors;
use eszip::EszipRelativeFileBaseUrl;
use rand::Rng;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::installer::infer_name_from_url;

pub async fn compile(
  flags: Arc<Flags>,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
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
        "The compiled executable may encounter runtime errors.",
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

  let ts_config_for_emit = cli_options
    .resolve_ts_config_for_emit(deno_config::deno_json::TsConfigType::Emit)?;
  check_warn_tsconfig(&ts_config_for_emit);
  let (transpile_options, emit_options) =
    crate::args::ts_config_to_transpile_and_emit_options(
      ts_config_for_emit.ts_config,
    )?;
  let parser = parsed_source_cache.as_capturing_parser();
  let root_dir_url = resolve_root_dir_from_specifiers(
    cli_options.workspace().root_dir(),
    graph.specifiers().map(|(s, _)| s).chain(
      cli_options
        .node_modules_dir_path()
        .and_then(|p| ModuleSpecifier::from_directory_path(p).ok())
        .iter(),
    ),
  );
  log::debug!("Binary root dir: {}", root_dir_url);
  let root_dir_url = EszipRelativeFileBaseUrl::new(&root_dir_url);
  let eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
    graph,
    parser,
    transpile_options,
    emit_options,
    // make all the modules relative to the root folder
    relative_file_base: Some(root_dir_url),
    npm_packages: None,
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

  let file = std::fs::File::create(&temp_path).with_context(|| {
    format!("Opening temporary file '{}'", temp_path.display())
  })?;

  let write_result = binary_writer
    .write_bin(
      file,
      eszip,
      root_dir_url,
      module_specifier,
      &compile_flags,
      cli_options,
    )
    .await
    .with_context(|| {
      format!("Writing temporary file '{}'", temp_path.display())
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

fn resolve_root_dir_from_specifiers<'a>(
  starting_dir: &ModuleSpecifier,
  specifiers: impl Iterator<Item = &'a ModuleSpecifier>,
) -> ModuleSpecifier {
  fn select_common_root<'a>(a: &'a str, b: &'a str) -> &'a str {
    let min_length = a.len().min(b.len());

    let mut last_slash = 0;
    for i in 0..min_length {
      if a.as_bytes()[i] == b.as_bytes()[i] && a.as_bytes()[i] == b'/' {
        last_slash = i;
      } else if a.as_bytes()[i] != b.as_bytes()[i] {
        break;
      }
    }

    // Return the common root path up to the last common slash.
    // This returns a slice of the original string 'a', up to and including the last matching '/'.
    let common = &a[..=last_slash];
    if cfg!(windows) && common == "file:///" {
      a
    } else {
      common
    }
  }

  fn is_file_system_root(url: &str) -> bool {
    let Some(path) = url.strip_prefix("file:///") else {
      return false;
    };
    if cfg!(windows) {
      let Some((_drive, path)) = path.split_once('/') else {
        return true;
      };
      path.is_empty()
    } else {
      path.is_empty()
    }
  }

  let mut found_dir = starting_dir.as_str();
  if !is_file_system_root(found_dir) {
    for specifier in specifiers {
      if specifier.scheme() == "file" {
        found_dir = select_common_root(found_dir, specifier.as_str());
      }
    }
  }
  let found_dir = if is_file_system_root(found_dir) {
    found_dir
  } else {
    // include the parent dir name because it helps create some context
    found_dir
      .strip_suffix('/')
      .unwrap_or(found_dir)
      .rfind('/')
      .map(|i| &found_dir[..i + 1])
      .unwrap_or(found_dir)
  };
  ModuleSpecifier::parse(found_dir).unwrap()
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
        include: vec![],
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
        icon: None,
        no_terminal: false,
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

  #[test]
  fn test_resolve_root_dir_from_specifiers() {
    fn resolve(start: &str, specifiers: &[&str]) -> String {
      let specifiers = specifiers
        .iter()
        .map(|s| ModuleSpecifier::parse(s).unwrap())
        .collect::<Vec<_>>();
      resolve_root_dir_from_specifiers(
        &ModuleSpecifier::parse(start).unwrap(),
        specifiers.iter(),
      )
      .to_string()
    }

    assert_eq!(resolve("file:///a/b/c", &["file:///a/b/c/d"]), "file:///a/");
    assert_eq!(
      resolve("file:///a/b/c/", &["file:///a/b/c/d"]),
      "file:///a/b/"
    );
    assert_eq!(
      resolve("file:///a/b/c/", &["file:///a/b/c/d", "file:///a/b/c/e"]),
      "file:///a/b/"
    );
    assert_eq!(resolve("file:///", &["file:///a/b/c/d"]), "file:///");
    if cfg!(windows) {
      assert_eq!(resolve("file:///c:/", &["file:///c:/test"]), "file:///c:/");
      // this will ignore the other one because it's on a separate drive
      assert_eq!(
        resolve("file:///c:/a/b/c/", &["file:///v:/a/b/c/d"]),
        "file:///c:/a/b/"
      );
    }
  }
}
