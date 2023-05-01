// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::io::AllowStdIo;
use deno_core::futures::AsyncReadExt;
use deno_core::futures::AsyncSeekExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::permissions::PermissionsOptions;
use log::Level;
use serde::Deserialize;
use serde::Serialize;

use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::CompileFlags;
use crate::cache::DenoDir;
use crate::file_fetcher::FileFetcher;
use crate::http_util::HttpClient;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

const MAGIC_TRAILER: &[u8; 8] = b"d3n0l4nd";

#[derive(Deserialize, Serialize)]
pub struct Metadata {
  pub argv: Vec<String>,
  pub unstable: bool,
  pub seed: Option<u64>,
  pub permissions: PermissionsOptions,
  pub location: Option<Url>,
  pub v8_flags: Vec<String>,
  pub log_level: Option<Level>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub maybe_import_map: Option<(Url, String)>,
  pub entrypoint: ModuleSpecifier,
}

pub fn write_binary_bytes(
  writer: &mut impl Write,
  original_bin: Vec<u8>,
  metadata: &Metadata,
  eszip: eszip::EszipV2,
) -> Result<(), AnyError> {
  let metadata = serde_json::to_string(metadata)?.as_bytes().to_vec();
  let eszip_archive = eszip.into_bytes();

  let eszip_pos = original_bin.len();
  let metadata_pos = eszip_pos + eszip_archive.len();
  let mut trailer = MAGIC_TRAILER.to_vec();
  trailer.write_all(&eszip_pos.to_be_bytes())?;
  trailer.write_all(&metadata_pos.to_be_bytes())?;

  writer.write_all(&original_bin)?;
  writer.write_all(&eszip_archive)?;
  writer.write_all(&metadata)?;
  writer.write_all(&trailer)?;

  Ok(())
}

pub fn is_standalone_binary(exe_path: &Path) -> bool {
  let Ok(mut output_file) = std::fs::File::open(exe_path) else {
    return false;
  };
  if output_file.seek(SeekFrom::End(-24)).is_err() {
    // This seek may fail because the file is too small to possibly be
    // `deno compile` output.
    return false;
  }
  let mut trailer = [0; 24];
  if output_file.read_exact(&mut trailer).is_err() {
    return false;
  };
  let (magic_trailer, _) = trailer.split_at(8);
  magic_trailer == MAGIC_TRAILER
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by checking for the magic trailer string `d3n0l4nd` at EOF-24 (8 bytes * 3).
/// The magic trailer is followed by:
/// - a u64 pointer to the JS bundle embedded in the binary
/// - a u64 pointer to JSON metadata (serialized flags) embedded in the binary
/// These are dereferenced, and the bundle is executed under the configuration
/// specified by the metadata. If no magic trailer is present, this function
/// exits with `Ok(None)`.
pub async fn extract_standalone(
  exe_path: &Path,
  cli_args: Vec<String>,
) -> Result<Option<(Metadata, eszip::EszipV2)>, AnyError> {
  let file = std::fs::File::open(exe_path)?;

  let mut bufreader =
    deno_core::futures::io::BufReader::new(AllowStdIo::new(file));

  let trailer_pos = bufreader.seek(SeekFrom::End(-24)).await?;
  let mut trailer = [0; 24];
  bufreader.read_exact(&mut trailer).await?;
  let (magic_trailer, rest) = trailer.split_at(8);
  if magic_trailer != MAGIC_TRAILER {
    return Ok(None);
  }

  let (eszip_archive_pos, rest) = rest.split_at(8);
  let metadata_pos = rest;
  let eszip_archive_pos = u64_from_bytes(eszip_archive_pos)?;
  let metadata_pos = u64_from_bytes(metadata_pos)?;
  let metadata_len = trailer_pos - metadata_pos;

  bufreader.seek(SeekFrom::Start(eszip_archive_pos)).await?;

  let (eszip, loader) = eszip::EszipV2::parse(bufreader)
    .await
    .context("Failed to parse eszip header")?;

  let mut bufreader = loader.await.context("Failed to parse eszip archive")?;

  bufreader.seek(SeekFrom::Start(metadata_pos)).await?;

  let mut metadata = String::new();

  bufreader
    .take(metadata_len)
    .read_to_string(&mut metadata)
    .await
    .context("Failed to read metadata from the current executable")?;

  let mut metadata: Metadata = serde_json::from_str(&metadata).unwrap();
  metadata.argv.append(&mut cli_args[1..].to_vec());

  Ok(Some((metadata, eszip)))
}

fn u64_from_bytes(arr: &[u8]) -> Result<u64, AnyError> {
  let fixed_arr: &[u8; 8] = arr
    .try_into()
    .context("Failed to convert the buffer into a fixed-size array")?;
  Ok(u64::from_be_bytes(*fixed_arr))
}

pub struct DenoCompileBinaryWriter<'a> {
  file_fetcher: &'a FileFetcher,
  client: &'a HttpClient,
  deno_dir: &'a DenoDir,
}

impl<'a> DenoCompileBinaryWriter<'a> {
  pub fn new(
    file_fetcher: &'a FileFetcher,
    client: &'a HttpClient,
    deno_dir: &'a DenoDir,
  ) -> Self {
    Self {
      file_fetcher,
      client,
      deno_dir,
    }
  }

  pub async fn write_bin(
    &self,
    writer: &mut impl Write,
    eszip: eszip::EszipV2,
    module_specifier: &ModuleSpecifier,
    compile_flags: &CompileFlags,
    cli_options: &CliOptions,
  ) -> Result<(), AnyError> {
    // Select base binary based on target
    let original_binary =
      self.get_base_binary(compile_flags.target.clone()).await?;

    self
      .write_standalone_binary(
        writer,
        original_binary,
        eszip,
        module_specifier,
        cli_options,
        compile_flags,
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
  async fn write_standalone_binary(
    &self,
    writer: &mut impl Write,
    original_bin: Vec<u8>,
    eszip: eszip::EszipV2,
    entrypoint: &ModuleSpecifier,
    cli_options: &CliOptions,
    compile_flags: &CompileFlags,
  ) -> Result<(), AnyError> {
    let ca_data = match cli_options.ca_data() {
      Some(CaData::File(ca_file)) => Some(
        std::fs::read(ca_file)
          .with_context(|| format!("Reading: {ca_file}"))?,
      ),
      Some(CaData::Bytes(bytes)) => Some(bytes.clone()),
      None => None,
    };
    let maybe_import_map = cli_options
      .resolve_import_map(self.file_fetcher)
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

    write_binary_bytes(writer, original_bin, &metadata, eszip)
  }
}
