// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { loadSync } from "./mod.ts";

if (!(Deno.readTextFileSync instanceof Function)) {
  // Avoid errors that occur in deno deploy: https://github.com/denoland/deno_std/issues/1957
  console.warn(
    `Deno.readTextFileSync is not a function: No .env data was read.`,
  );
} else {
  loadSync({ export: true });
}
