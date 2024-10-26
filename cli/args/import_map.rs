// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;

use crate::file_fetcher::FileFetcher;

pub async fn resolve_import_map_value_from_specifier(
  specifier: &Url,
  file_fetcher: &FileFetcher,
) -> Result<serde_json::Value, AnyError> {
  if specifier.scheme() == "data" {
    let data_url_text =
      deno_graph::source::RawDataUrl::parse(specifier)?.decode()?;
    Ok(serde_json::from_str(&data_url_text)?)
  } else {
    let file = file_fetcher
      .fetch_bypass_permissions(specifier)
      .await?
      .into_text_decoded()?;
    Ok(serde_json::from_str(&file.source)?)
  }
}
