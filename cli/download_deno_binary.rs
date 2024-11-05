// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use crate::http_util::HttpClient;
use crate::http_util::HttpClientProvider;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use crate::cache::DenoDir;
use crate::shared::ReleaseChannel;

#[derive(Clone, Copy, Debug)]
pub enum BinaryKind {
  Deno,
  Denort,
}

impl BinaryKind {
  pub fn name(&self) -> &str {
    match self {
      BinaryKind::Deno => "deno",
      BinaryKind::Denort => "denort",
    }
  }
}

pub async fn download_deno_binary(
  http_client_provider: &HttpClientProvider,
  deno_dir: &DenoDir,
  binary_kind: BinaryKind,
  target: &str,
  version_or_git_hash: &str,
  release_channel: ReleaseChannel,
) -> Result<PathBuf, AnyError> {
  let binary_name = archive_name(binary_kind, target);
  let binary_path_suffix = match release_channel {
    ReleaseChannel::Canary => {
      format!("canary/{}/{}", version_or_git_hash, binary_name,)
    }
    _ => {
      format!("release/v{}/{}", version_or_git_hash, binary_name)
    }
  };

  let download_directory = deno_dir.dl_folder_path();
  let binary_path = download_directory.join(&binary_path_suffix);

  if !binary_path.exists() {
    let http_client = http_client_provider.get_or_create()?;
    download_base_binary(
      &http_client,
      &download_directory,
      &binary_path_suffix,
    )
    .await?;
  }

  Ok(binary_path)
}

pub fn archive_name(binary_kind: BinaryKind, target: &str) -> String {
  format!("{}-{}.zip", binary_kind.name(), target)
}

async fn download_base_binary(
  http_client: &HttpClient,
  output_directory: &Path,
  binary_path_suffix: &str,
) -> Result<(), AnyError> {
  let download_url = format!("https://dl.deno.land/{binary_path_suffix}");
  let maybe_bytes = {
    let progress_bars = ProgressBar::new(ProgressBarStyle::DownloadBars);
    // provide an empty string here in order to prefer the downloading
    // text above which will stay alive after the progress bars are complete
    let progress = progress_bars.update("");
    http_client
      .download_with_progress_and_retries(
        download_url.parse()?,
        None,
        &progress,
      )
      .await?
  };
  let Some(bytes) = maybe_bytes else {
    bail!("Failed downloading {download_url}. The version you requested may not have been built for the current architecture.");
  };

  std::fs::create_dir_all(output_directory)?;
  let output_path = output_directory.join(binary_path_suffix);
  std::fs::create_dir_all(output_path.parent().unwrap())?;
  tokio::fs::write(output_path, bytes).await?;
  Ok(())
}
