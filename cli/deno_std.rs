// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;

// WARNING: Ensure this is the only deno_std version reference as this
// is automatically updated by the version bump workflow.
pub(crate) static STD_URL_STR: &str = "https://deno.land/std@0.153.0/";

pub(crate) static STD_URL: Lazy<Url> =
  Lazy::new(|| Url::parse(STD_URL_STR).unwrap());
