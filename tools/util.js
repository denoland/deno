// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { dirname, join } from "https://deno.land/std@0.76.0/path/mod.ts";
export { dirname, join };
export { existsSync } from "https://deno.land/std@0.76.0/fs/mod.ts";

const ROOT_PATH = dirname(dirname(import.meta.url));

function buildMode() {
  if (Deno.args.contains("--release")) {
    return "release";
  }

  return "debug";
}

export function buildPath() {
  join(ROOT_PATH, "target", buildMode());
}
