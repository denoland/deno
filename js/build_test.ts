// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function buildInfo() {
  // Deno.build is injected by rollup at compile time. Here
  // we check it has been properly transformed.
  const { arch, os } = Deno.build;
  assert(arch === "x64");
  assert(os === "mac" || os === "win" || os === "linux");
});

test(function buildGnArgs() {
  assert(Deno.build.args.length > 100);
});
