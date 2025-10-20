// Copyright 2018-2025 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/30923
// This test ensures that Deno.openKv works with relative paths
// when run with -A flag, even if the DB file doesn't exist yet.

// This should create the DB file successfully
// Before the fix (deno_path_util <= 0.6.1), this would fail with:
// "NotFound: No such file or directory (os error 2)"
using db = await Deno.openKv("test.kv");

console.log("Database opened successfully");
