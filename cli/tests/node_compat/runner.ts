// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import "./polyfill_globals.js";
import { createRequire } from "node:module";
import { toFileUrl } from "../../../test_util/std/path/mod.ts";
const file = Deno.args[0];
if (!file) {
  throw new Error("No file provided");
}

if (file.endsWith(".mjs")) {
  await import(toFileUrl(file).href);
} else {
  createRequire(import.meta.url)(file);
}
