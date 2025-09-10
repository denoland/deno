#!/usr/bin/env -S deno run -RW

// Copyright 2018-2025 the Deno authors. MIT license.

/**
 * This script generates the `import_map.json` file based on `tests/util/std/*` packages.
 *
 * Run this script when you updated the git submodule at `tests/util/std`.
 */

import denoConfig from "../tests/util/std/deno.json" with { type: "json" };
import { join } from "jsr:@std/path@1/posix/join";

const importMap: { imports: Record<string, string> } = { imports: {} };
for (const workspace of denoConfig.workspace) {
  const { default: pkgDenoConfig } = await import(
    `../tests/util/std/${workspace}/deno.json`,
    {
      with: { type: "json" },
    }
  );

  if (typeof pkgDenoConfig.exports === "string") {
    importMap.imports[pkgDenoConfig.name] = `./${
      join(workspace, pkgDenoConfig.exports)
    }`;
  } else {
    for (
      const [moduleName, modulePath] of Object.entries(
        pkgDenoConfig.exports as Record<string, string>,
      )
    ) {
      importMap.imports[join(pkgDenoConfig.name, moduleName)] =
        `./tests/util/std/${join(workspace, modulePath)}`;
    }
  }
}

await Deno.writeTextFile("./import_map.json", JSON.stringify(importMap, null, 2));

