// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CompileFlags;
use crate::args::Flags;
use crate::cache::DenoDir;
use crate::graph_util::create_graph_and_maybe_check;
use crate::graph_util::error_for_any_npm_specifier;
use crate::http_util::HttpClient;
use crate::standalone::Metadata;
use crate::standalone::MAGIC_TRAILER;
use crate::util::path::path_has_trailing_slash;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::ProcState;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::ModuleSpecifier;
use deno_runtime::colors;
use deno_runtime::permissions::PermissionsContainer;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::installer::infer_name_from_url;

pub async fn compile(
  flags: Flags,
  compile_flags: CompileFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::build(flags.clone()).await?;
  let module_specifier = resolve_url_or_path(&compile_flags.source_file)?;
  let deno_dir = &ps.dir;

  let output_path = resolve_compile_executable_output_path(&compile_flags)?;

  let graph = Arc::try_unwrap(
    create_graph_and_maybe_check(module_specifier.clone(), &ps).await?,
  )
  .unwrap();

  // at the moment, we don't support npm specifiers in deno_compile, so show an error
  error_for_any_npm_specifier(&graph)?;

  graph.valid()?;

  let parser = ps.parsed_source_cache.as_capturing_parser();
  let eszip = eszip::EszipV2::from_graph(graph, &parser, Default::default())?;

  log::info!(
    "{} {}",
    colors::green("Compile"),
    module_specifier.to_string()
  );

  // Select base binary based on target
  let original_binary =
    get_base_binary(&ps.http_client, deno_dir, compile_flags.target.clone())
      .await?;

  let final_bin = create_standalone_binary(
    original_binary,
    eszip,
    module_specifier.clone(),
    &compile_flags,
    ps,
  )
  .await?;

  log::info!("{} {}", colors::green("Emit"), output_path.display());

  write_standalone_binary(output_path, final_bin).await?;
  Ok(())
}

async fn get_base_binary(
  client: &HttpClient,
  deno_dir: &DenoDir,
  target: Option<String>,
) -> Result<Vec<u8>, AnyError> {
  if target.is_none() {
    let path = std::env::current_exe()?;
    return Ok(tokio::fs::read(path).await?);
  }

  let target = target.unwrap_or_else(|| env!("TARGET").to_string());
  let binary_name = format!("deno-{}.zip", target);

  let binary_path_suffix = if crate::version::is_canary() {
    format!("canary/{}/{}", crate::version::GIT_COMMIT_HASH, binary_name)
  } else {
    format!("release/v{}/{}", env!("CARGO_PKG_VERSION"), binary_name)
  };

  let download_directory = deno_dir.dl_folder_path();
  let binary_path = download_directory.join(&binary_path_suffix);

  if !binary_path.exists() {
    download_base_binary(client, &download_directory, &binary_path_suffix)
      .await?;
  }

  let archive_data = tokio::fs::read(binary_path).await?;
  let base_binary_path =
    crate::tools::upgrade::unpack(archive_data, target.contains("windows"))?;
  let base_binary = tokio::fs::read(base_binary_path).await?;
  Ok(base_binary)
}

async fn download_base_binary(
  client: &HttpClient,
  output_directory: &Path,
  binary_path_suffix: &str,
) -> Result<(), AnyError> {
  let download_url = format!("https://dl.deno.land/{}", binary_path_suffix);
  let maybe_bytes = {
    let progress_bars = ProgressBar::new(ProgressBarStyle::DownloadBars);
    let progress = progress_bars.update(&download_url);

    client
      .download_with_progress(download_url, &progress)
      .await?
  };
  let bytes = match maybe_bytes {
    Some(bytes) => bytes,
    None => {
      log::info!("Download could not be found, aborting");
      std::process::exit(1)
    }
  };

  std::fs::create_dir_all(output_directory)?;
  let output_path = output_directory.join(binary_path_suffix);
  std::fs::create_dir_all(output_path.parent().unwrap())?;
  tokio::fs::write(output_path, bytes).await?;
  Ok(())
}

/// This functions creates a standalone deno binary by appending a bundle
/// and magic trailer to the currently executing binary.
async fn create_standalone_binary(
  mut original_bin: Vec<u8>,
  eszip: eszip::EszipV2,
  entrypoint: ModuleSpecifier,
  compile_flags: &CompileFlags,
  ps: ProcState,
) -> Result<Vec<u8>, AnyError> {
  let mut eszip_archive = eszip.into_bytes();

  let ca_data = match ps.options.ca_file() {
    Some(ca_file) => {
      Some(fs::read(ca_file).with_context(|| format!("Reading: {}", ca_file))?)
    }
    None => None,
  };
  let maybe_import_map: Option<(Url, String)> =
    match ps.options.resolve_import_map_specifier()? {
      None => None,
      Some(import_map_specifier) => {
        let file = ps
          .file_fetcher
          .fetch(&import_map_specifier, PermissionsContainer::allow_all())
          .await
          .context(format!(
            "Unable to load '{}' import map",
            import_map_specifier
          ))?;

        Some((import_map_specifier, file.source.to_string()))
      }
    };
  let metadata = Metadata {
    argv: compile_flags.args.clone(),
    unstable: ps.options.unstable(),
    seed: ps.options.seed(),
    location: ps.options.location_flag().clone(),
    permissions: ps.options.permissions_options(),
    v8_flags: ps.options.v8_flags().clone(),
    unsafely_ignore_certificate_errors: ps
      .options
      .unsafely_ignore_certificate_errors()
      .clone(),
    log_level: ps.options.log_level(),
    ca_stores: ps.options.ca_stores().clone(),
    ca_data,
    entrypoint,
    maybe_import_map,
  };
  let mut metadata = serde_json::to_string(&metadata)?.as_bytes().to_vec();

  let eszip_pos = original_bin.len();
  let metadata_pos = eszip_pos + eszip_archive.len();
  let mut trailer = MAGIC_TRAILER.to_vec();
  trailer.write_all(&eszip_pos.to_be_bytes())?;
  trailer.write_all(&metadata_pos.to_be_bytes())?;

  let mut final_bin = Vec::with_capacity(
    original_bin.len() + eszip_archive.len() + trailer.len(),
  );
  final_bin.append(&mut original_bin);
  final_bin.append(&mut eszip_archive);
  final_bin.append(&mut metadata);
  final_bin.append(&mut trailer);

  Ok(final_bin)
}

/// This function writes out a final binary to specified path. If output path
/// is not already standalone binary it will return error instead.
async fn write_standalone_binary(
  output_path: PathBuf,
  final_bin: Vec<u8>,
) -> Result<(), AnyError> {
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

    // Make sure we don't overwrite any file not created by Deno compiler.
    // Check for magic trailer in last 24 bytes.
    let mut has_trailer = false;
    let mut output_file = File::open(&output_path)?;
    // This seek may fail because the file is too small to possibly be
    // `deno compile` output.
    if output_file.seek(SeekFrom::End(-24)).is_ok() {
      let mut trailer = [0; 24];
      output_file.read_exact(&mut trailer)?;
      let (magic_trailer, _) = trailer.split_at(8);
      has_trailer = magic_trailer == MAGIC_TRAILER;
    }
    if !has_trailer {
      bail!(
        concat!(
          "Could not compile to file '{}' because the file already exists ",
          "and cannot be overwritten. Please delete the existing file or ",
          "use the `--output <file-path` flag to provide an alternative name."
        ),
        output_path.display()
      );
    }

    // Remove file if it was indeed a deno compiled binary, to avoid corruption
    // (see https://github.com/denoland/deno/issues/10310)
    std::fs::remove_file(&output_path)?;
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
    tokio::fs::create_dir_all(output_base).await?;
  }

  tokio::fs::write(&output_path, final_bin).await?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o777);
    tokio::fs::set_permissions(output_path, perms).await?;
  }

  Ok(())
}

fn resolve_compile_executable_output_path(
  compile_flags: &CompileFlags,
) -> Result<PathBuf, AnyError> {
  let module_specifier = resolve_url_or_path(&compile_flags.source_file)?;
  compile_flags.output.as_ref().and_then(|output| {
    if path_has_trailing_slash(output) {
      let infer_file_name = infer_name_from_url(&module_specifier).map(PathBuf::from)?;
      Some(output.join(infer_file_name))
    } else {
      Some(output.to_path_buf())
    }
  }).or_else(|| {
    infer_name_from_url(&module_specifier).map(PathBuf::from)
  }).ok_or_else(|| generic_error(
    "An executable name was not provided. One could not be inferred from the URL. Aborting.",
  )).map(|output| {
    get_os_specific_filepath(output, &compile_flags.target)
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

  #[test]
  fn resolve_compile_executable_output_path_target_linux() {
    let path = resolve_compile_executable_output_path(&CompileFlags {
      source_file: "mod.ts".to_string(),
      output: Some(PathBuf::from("./file")),
      args: Vec::new(),
      target: Some("x86_64-unknown-linux-gnu".to_string()),
    })
    .unwrap();

    // no extension, no matter what the operating system is
    // because the target was specified as linux
    // https://github.com/denoland/deno/issues/9667
    assert_eq!(path.file_name().unwrap(), "file");
  }

  #[test]
  fn resolve_compile_executable_output_path_target_windows() {
    let path = resolve_compile_executable_output_path(&CompileFlags {
      source_file: "mod.ts".to_string(),
      output: Some(PathBuf::from("./file")),
      args: Vec::new(),
      target: Some("x86_64-pc-windows-msvc".to_string()),
    })
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
