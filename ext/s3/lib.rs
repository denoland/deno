// Copyright 2018-2026 the Deno authors. MIT license.

pub const UNSTABLE_FEATURE_NAME: &str = "s3";

deno_core::extension!(deno_s3, lazy_loaded_js = ["01_s3.ts"],);
