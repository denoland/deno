// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import "./polyfill_globals.js";
import { createRequire } from "node:module";
<<<<<<< HEAD
import { toFileUrl } from "../../../test_util/std/path/mod.ts";
=======
>>>>>>> 8c07f52a7 (1.38.4 (#21398))
const file = Deno.args[0];
if (!file) {
  throw new Error("No file provided");
}
<<<<<<< HEAD

if (file.endsWith(".mjs")) {
  await import(toFileUrl(file).href);
} else {
  createRequire(import.meta.url)(file);
}
=======
createRequire(import.meta.url)(file);
>>>>>>> 8c07f52a7 (1.38.4 (#21398))
