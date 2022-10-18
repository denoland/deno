// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;
use once_cell::sync::Lazy;

// WARNING: Ensure this is the only deno_std version reference as this
// is automatically updated by the version bump workflow.
static CURRENT_STD_URL_STR: &str = "https://deno.land/std@0.160.0/";

pub static CURRENT_STD_URL: Lazy<Url> =
  Lazy::new(|| Url::parse(CURRENT_STD_URL_STR).unwrap());
