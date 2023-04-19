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

pub fn get_binary_bytes(
  mut original_bin: Vec<u8>,
  metadata: &Metadata,
  eszip: eszip::EszipV2,
) -> Result<Vec<u8>, AnyError> {
  let mut metadata = serde_json::to_string(metadata)?.as_bytes().to_vec();
  let mut eszip_archive = eszip.into_bytes();

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

pub fn is_compiled_binary(exe_path: &Path) -> bool {
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
