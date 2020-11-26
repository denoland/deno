// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, unitTest } from "./test_util.ts";

unitTest(function buildInfo(): void {
  // Deno.build is injected by rollup at compile time. Here
  // we check it has been properly transformed.
  const { arch, os } = Deno.build;
  assert(arch.length > 0);
  assert(os === "darwin" || os === "windows" || os === "linux");
});
