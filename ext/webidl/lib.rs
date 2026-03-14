// Copyright 2018-2026 the Deno authors. MIT license.

#![forbid(clippy::disallowed_methods)]

deno_core::extension!(deno_webidl, esm = ["00_webidl.js"],);
