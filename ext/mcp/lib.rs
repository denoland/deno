// Copyright 2018-2026 the Deno authors. MIT license.

pub const UNSTABLE_FEATURE_NAME: &str = "mcp";

deno_core::extension!(deno_mcp, lazy_loaded_js = ["01_mcp.ts"],);
