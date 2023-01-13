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

    // rust
    "*.rs",
    ":!:ops/optimizer_tests/**",

    // toml
    "*Cargo.toml",
  ]);

  let totalCount = 0;
  const sourceFilesSet = new Set(sourceFiles);

  for (const file of sourceFilesSet) {
    const ERROR_MSG = "Copyright header is missing: ";

    const fileText = await readFirstPartOfFile(file);
    if (file.endsWith("Cargo.toml")) {
      if (
        !fileText.startsWith(
          "# Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.",
        )
      ) {
        console.log(ERROR_MSG + file);
        totalCount += 1;
      }
      continue;
    }

    // use .includes(...) because the first line might be a shebang
    if (
      !fileText.includes(
        "// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.",
      )
    ) {
      console.log(ERROR_MSG + file);
      totalCount += 1;
    }
  }

  if (totalCount > 0) {
    throw new Error(`Copyright checker had ${totalCount} errors.`);
  }
}

if (import.meta.main) {
  await checkCopyright();
}
