// Copyright 2018-2025 the Deno authors. MIT license.
import { assert } from "./test_util.ts";

Deno.test(function buildInfo() {
  // Deno.build is injected by rollup at compile time. Here
  // we check it has been properly transformed.
  const { arch, os } = Deno.build;
  assert(arch.length > 0);
  assert(os === "darwin" || os === "windows" || os === "linux");
});
