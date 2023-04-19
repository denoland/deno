use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_runtime::colors;

use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::cache::DenoDir;
use crate::file_fetcher::FileFetcher;
use crate::http_util::HttpClient;
use crate::proc_state::ProcState;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

use super::virtual_fs::VfsBuilder;
use super::Metadata;
use super::MAGIC_TRAILER;

pub struct DenoCompileBinaryBuilder {
  file_fetcher: Arc<FileFetcher>,
  client: HttpClient,
  deno_dir: DenoDir,
}

impl DenoCompileBinaryBuilder {
  pub fn new(
    file_fetcher: Arc<FileFetcher>,
    client: HttpClient,
    deno_dir: DenoDir,
  ) -> Self {
    Self {
      file_fetcher,
      client,
      deno_dir,
    }
  }

  pub async fn build_bin(
    &self,
    eszip: eszip::EszipV2,
    module_specifier: &ModuleSpecifier,
    compile_flags: &CompileFlags,
    cli_options: &CliOptions,
  ) -> Result<Vec<u8>, AnyError> {
    // Select base binary based on target
    let original_binary =
      self.get_base_binary(compile_flags.target.clone()).await?;

    self
      .create_standalone_binary(
        original_binary,
        eszip,
        module_specifier,
        cli_options,
        &compile_flags,
      )
      .await
  }

  async fn get_base_binary(
    &self,
    target: Option<String>,
  ) -> Result<Vec<u8>, AnyError> {
    if target.is_none() {
      let path = std::env::current_exe()?;
      return Ok(std::fs::read(path)?);
    }

    let target = target.unwrap_or_else(|| env!("TARGET").to_string());
    let binary_name = format!("deno-{target}.zip");

    let binary_path_suffix = if crate::version::is_canary() {
      format!("canary/{}/{}", crate::version::GIT_COMMIT_HASH, binary_name)
    } else {
      format!("release/v{}/{}", env!("CARGO_PKG_VERSION"), binary_name)
    };

    let download_directory = self.deno_dir.dl_folder_path();
    let binary_path = download_directory.join(&binary_path_suffix);

    if !binary_path.exists() {
      self
        .download_base_binary(&download_directory, &binary_path_suffix)
        .await?;
    }

    let archive_data = std::fs::read(binary_path)?;
    let temp_dir = tempfile::TempDir::new()?;
    let base_binary_path = crate::tools::upgrade::unpack_into_dir(
      archive_data,
      target.contains("windows"),
      &temp_dir,
    )?;
    let base_binary = std::fs::read(base_binary_path)?;
    drop(temp_dir); // delete the temp dir
    Ok(base_binary)
  }

  async fn download_base_binary(
    &self,
    output_directory: &Path,
    binary_path_suffix: &str,
  ) -> Result<(), AnyError> {
    let download_url = format!("https://dl.deno.land/{binary_path_suffix}");
    let maybe_bytes = {
      let progress_bars = ProgressBar::new(ProgressBarStyle::DownloadBars);
      let progress = progress_bars.update(&download_url);

      self
        .client
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
    &self,
    mut original_bin: Vec<u8>,
    eszip: eszip::EszipV2,
    entrypoint: &ModuleSpecifier,
    cli_options: &CliOptions,
    compile_flags: &CompileFlags,
  ) -> Result<Vec<u8>, AnyError> {
    let mut eszip_archive = eszip.into_bytes();

    let ca_data = match cli_options.ca_data() {
      Some(CaData::File(ca_file)) => Some(
        std::fs::read(ca_file)
          .with_context(|| format!("Reading: {ca_file}"))?,
      ),
      Some(CaData::Bytes(bytes)) => Some(bytes.clone()),
      None => None,
    };
    let maybe_import_map = cli_options
      .resolve_import_map(&self.file_fetcher)
      .await?
      .map(|import_map| (import_map.base_url().clone(), import_map.to_json()));
    let metadata = Metadata {
      argv: compile_flags.args.clone(),
      unstable: cli_options.unstable(),
      seed: cli_options.seed(),
      location: cli_options.location_flag().clone(),
      permissions: cli_options.permissions_options(),
      v8_flags: cli_options.v8_flags().clone(),
      unsafely_ignore_certificate_errors: cli_options
        .unsafely_ignore_certificate_errors()
        .clone(),
      log_level: cli_options.log_level(),
      ca_stores: cli_options.ca_stores().clone(),
      ca_data,
      entrypoint: entrypoint.clone(),
      maybe_import_map,
    };
    let mut metadata = serde_json::to_string(&metadata)?.as_bytes().to_vec();

    let eszip_pos = original_bin.len();
    let metadata_pos = eszip_pos + eszip_archive.len();
    let mut trailer = MAGIC_TRAILER.to_vec();
    trailer.write_all(&eszip_pos.to_be_bytes())?;
    trailer.write_all(&metadata_pos.to_be_bytes())?;

    let mut final_bin = Vec::with_capacity(
      original_bin.len() + eszip_archive.len() + metadata.len() + trailer.len(),
    );
    final_bin.append(&mut original_bin);
    final_bin.append(&mut eszip_archive);
    final_bin.append(&mut metadata);
    final_bin.append(&mut trailer);

    Ok(final_bin)
  }

  /// This function writes out a final binary to specified path. If output path
  /// is not already standalone binary it will return error instead.
  pub fn write(
    &self,
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
      let mut output_file = std::fs::File::open(&output_path)?;
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
      std::fs::create_dir_all(output_base)?;
    }

    std::fs::write(&output_path, final_bin)?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let perms = std::fs::Permissions::from_mode(0o777);
      std::fs::set_permissions(output_path, perms)?;
    }

    Ok(())
  }
}
