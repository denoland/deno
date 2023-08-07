#!/usr/bin/env -S deno run --unstable --allow-read=. --allow-run=git
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { getSources, ROOT_PATH } from "./util.js";

const buffer = new Uint8Array(1024);
const textDecoder = new TextDecoder();

async function readFirstPartOfFile(filePath) {
  const file = await Deno.open(filePath, { read: true });
  try {
    const byteCount = await file.read(buffer);
    return textDecoder.decode(buffer.slice(0, byteCount ?? 0));
  } finally {
    file.close();
  }
}

export async function checkCopyright() {
  const sourceFiles = await getSources(ROOT_PATH, [
    // js and ts
    "*.js",
    "*.ts",
    ":!:.github/mtime_cache/action.js",
    ":!:cli/tests/testdata/**",
    ":!:cli/bench/testdata/**",
    ":!:cli/tsc/dts/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:test_util/wpt/**",
    ":!:cli/tools/init/templates/**",
    ":!:cli/tests/unit_node/testdata/**",
    ":!:cli/tests/node_compat/test/**",
    ":!:cli/tools/bench/mitata.rs",

    // rust
    "*.rs",
    ":!:ops/optimizer_tests/**",

    // toml
    "*Cargo.toml",
  ]);

  const errors = [];
  const sourceFilesSet = new Set(sourceFiles);
  const ERROR_MSG = "Copyright header is missing: ";

  // Acceptable content before the copyright line
  const ACCEPTABLE_LINES =
    /^(\/\/ deno-lint-.*|\/\/ Copyright.*|\/\/ Ported.*|\s*|#!\/.*)\n/;
  const COPYRIGHT_LINE =
    "Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.";
  const TOML_COPYRIGHT_LINE = "# " + COPYRIGHT_LINE;
  const C_STYLE_COPYRIGHT_LINE = "// " + COPYRIGHT_LINE;

  for (const file of sourceFilesSet) {
    const fileText = await readFirstPartOfFile(file);
    if (file.endsWith("Cargo.toml")) {
      if (
        !fileText.startsWith(TOML_COPYRIGHT_LINE)
      ) {
        errors.push(ERROR_MSG + file);
      }
      continue;
    }

    if (
      !fileText.startsWith(C_STYLE_COPYRIGHT_LINE)
    ) {
      let trimmedText = fileText;
      // Attempt to trim accceptable lines
      while (
        ACCEPTABLE_LINES.test(trimmedText) &&
        !trimmedText.startsWith(C_STYLE_COPYRIGHT_LINE)
      ) {
        trimmedText = trimmedText.split("\n").slice(1).join("\n");
      }
      if (
        !trimmedText.startsWith(C_STYLE_COPYRIGHT_LINE)
      ) {
        errors.push(
          `${ERROR_MSG}${file} (incorrect line is '${
            trimmedText.split("\n", 1)
          }')`,
        );
      }
    }
  }

  if (errors.length > 0) {
    // show all the errors at the same time to prevent overlap with
    // other running scripts that may be outputting
    console.error(errors.join("\n"));
    throw new Error(`Copyright checker had ${errors.length} errors.`);
  }
}

if (import.meta.main) {
  await checkCopyright();
}
