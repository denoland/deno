#!/usr/bin/env -S deno run --allow-read=. --allow-run=git --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { getSources, ROOT_PATH } from "./util.js";

const copyrightYear = 2024;

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
    "*.mjs",
    "*.jsx",
    "*.ts",
    "*.tsx",
    ":!:.github/mtime_cache/action.js",
    ":!:cli/bench/testdata/**",
    ":!:cli/tools/bench/mitata.rs",
    ":!:cli/tools/init/templates/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:cli/tsc/dts/**",
    ":!:tests/node_compat/test/**",
    ":!:tests/registry/**",
    ":!:tests/specs/**",
    ":!:tests/testdata/**",
    ":!:tests/unit_node/testdata/**",
    ":!:tests/wpt/suite/**",

    // rust
    "*.rs",
    ":!:ops/optimizer_tests/**",

    // c
    "*.c",

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
    `Copyright 2018-${copyrightYear} the Deno authors. All rights reserved. MIT license.`;
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
      // Attempt to trim acceptable lines
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

  // check the main license file
  const licenseText = Deno.readTextFileSync(ROOT_PATH + "/LICENSE.md");
  if (
    !licenseText.includes(`Copyright 2018-${copyrightYear} the Deno authors`)
  ) {
    errors.push(`LICENSE.md has old copyright year`);
  }

  if (errors.length > 0) {
    // show all the errors at the same time to prevent overlap with
    // other running scripts that may be outputting
    console.error(errors.join("\n"));
    console.error(`Expected copyright:\n\`\`\`\n${COPYRIGHT_LINE}\n\`\`\``);
    throw new Error(`Copyright checker had ${errors.length} errors.`);
  }
}

if (import.meta.main) {
  await checkCopyright();
}
