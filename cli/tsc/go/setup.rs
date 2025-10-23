// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use deno_core::error::AnyError;
use deno_error::JsErrorBox;
use sha2::Digest;

use super::tsgo_version;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;

fn get_download_url(platform: &str) -> String {
  format!(
    "{}/typescript-go-{}-{}.zip",
    tsgo_version::DOWNLOAD_BASE_URL,
    tsgo_version::VERSION,
    platform
  )
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
pub enum DownloadError {
  #[error("unsupported platform for typescript-go: {0}")]
  UnsupportedPlatform(String),
  #[error("invalid download url: {0}")]
  InvalidDownloadUrl(String, #[source] deno_core::url::ParseError),
  #[error("failed to unpack typescript-go: {0}")]
  UnpackFailed(#[source] AnyError),
  #[error("failed to rename or copy typescript-go from {0} to {1}: {2}")]
  RenameOrCopyFailed(String, String, #[source] std::io::Error),
  #[error("failed to write zip file to {0}: {1}")]
  WriteZipFailed(String, #[source] std::io::Error),
  #[error("failed to download typescript-go: {0}")]
  DownloadFailed(#[source] crate::http_util::DownloadError),
  #[error("{0}")]
  HttpClient(#[source] JsErrorBox),
  #[error("failed to create temp directory: {0}")]
  CreateTempDirFailed(#[source] std::io::Error),
  #[error("hash mismatch: expected {0}, got {1}")]
  HashMismatch(String, String),
  #[error("binary not found: {0}")]
  BinaryNotFound(String),
}

fn verify_hash(platform: &str, data: &[u8]) -> Result<(), DownloadError> {
  let expected_hash = match platform {
    "windows-x64" => tsgo_version::HASHES.windows_x64,
    "macos-x64" => tsgo_version::HASHES.macos_x64,
    "macos-arm64" => tsgo_version::HASHES.macos_arm64,
    "linux-x64" => tsgo_version::HASHES.linux_x64,
    "linux-arm64" => tsgo_version::HASHES.linux_arm64,
    _ => unreachable!(),
  };
  let (algorithm, expected_hash) = expected_hash.split_once(':').unwrap();
  if algorithm != "sha256" {
    panic!("Hash algorithm is not sha256");
  }

  let mut hash = sha2::Sha256::new();
  hash.update(data);
  let hash = hash.finalize();

  let hash = faster_hex::hex_string(&hash);
  if hash != expected_hash {
    return Err(DownloadError::HashMismatch(expected_hash.to_string(), hash));
  }

  Ok(())
}

pub async fn ensure_tsgo(
  deno_dir: &DenoDir,
  http_client_provider: Arc<HttpClientProvider>,
) -> Result<&'static PathBuf, DownloadError> {
  static TSGO_PATH: OnceLock<PathBuf> = OnceLock::new();

  if let Some(bin_path) = TSGO_PATH.get() {
    return Ok(bin_path);
  }

  if let Ok(tsgo_path) = std::env::var("DENO_TSGO_PATH") {
    let tsgo_path = Path::new(&tsgo_path);
    if tsgo_path.exists() {
      return Ok(TSGO_PATH.get_or_init(|| PathBuf::from(tsgo_path)));
    } else {
      return Err(DownloadError::BinaryNotFound(
        tsgo_path.to_string_lossy().into_owned(),
      ));
    }
  }

  let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
    ("windows", "x86_64") => "windows-x64",
    ("macos", "x86_64") => "macos-x64",
    ("macos", "aarch64") => "macos-arm64",
    ("linux", "x86_64") => "linux-x64",
    ("linux", "aarch64") => "linux-arm64",
    _ => {
      return Err(DownloadError::UnsupportedPlatform(format!(
        "{} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
      )));
    }
  };

  let folder_path = deno_dir
    .dl_folder_path()
    .join(format!("tsgo-{}", tsgo_version::VERSION));

  let bin_path = folder_path.join(format!(
    "tsgo-{}{}",
    platform,
    if cfg!(windows) { ".exe" } else { "" }
  ));

  if bin_path.exists() {
    return Ok(TSGO_PATH.get_or_init(|| bin_path));
  }

  std::fs::create_dir_all(&folder_path)
    .map_err(DownloadError::CreateTempDirFailed)?;

  let client = http_client_provider
    .get_or_create()
    .map_err(DownloadError::HttpClient)?;
  let download_url = get_download_url(platform);
  log::debug!("Downloading tsgo from {}", download_url);
  let temp = tempfile::tempdir().map_err(DownloadError::CreateTempDirFailed)?;
  let path = temp.path().join("tsgo.zip");
  log::debug!("Downloading tsgo to {}", path.display());
  let data = client
    .download(
      deno_core::url::Url::parse(&download_url)
        .map_err(|e| DownloadError::InvalidDownloadUrl(download_url, e))?,
    )
    .await
    .map_err(DownloadError::DownloadFailed)?;

  verify_hash(platform, &data)?;

  std::fs::write(&path, &data).map_err(|e| {
    DownloadError::WriteZipFailed(path.display().to_string(), e)
  })?;

  log::debug!(
    "Unpacking tsgo from {} to {}",
    path.display(),
    temp.path().display()
  );
  let unpacked_path =
    crate::util::archive::unpack_into_dir(crate::util::archive::UnpackArgs {
      exe_name: "tsgo",
      archive_name: "tsgo.zip",
      archive_data: &data,
      is_windows: cfg!(windows),
      dest_path: temp.path(),
    })
    .map_err(DownloadError::UnpackFailed)?;
  std::fs::rename(&unpacked_path, &bin_path)
    .or_else(|_| std::fs::copy(&unpacked_path, &bin_path).map(|_| ()))
    .map_err(|e| {
      DownloadError::RenameOrCopyFailed(
        unpacked_path.to_string_lossy().into_owned(),
        bin_path.to_string_lossy().into_owned(),
        e,
      )
    })?;

  Ok(TSGO_PATH.get_or_init(|| bin_path))
}
