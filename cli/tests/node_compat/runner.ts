// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import "./polyfill_globals.js";
import { createRequire } from "node:module";
const file = Deno.args[0];
if (!file) {
  throw new Error("No file provided");
}
createRequire(import.meta.url)(file);
