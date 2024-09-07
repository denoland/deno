// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PredefinedColorSpace {
  Srgb,
  #[serde(rename = "display-p3")]
  DisplayP3,
}
