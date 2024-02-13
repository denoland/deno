// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/**
 * Running this script provides a list of suggested files that might be browser-compatible.
 * It skips test code, benchmark code, internal code and `version.ts` at the root.
 *
 * Run using: deno run --allow-read --allow-run _tools/check_browser_compat.ts
 */
import { walkSync } from "../fs/walk.ts";

const ROOT = new URL("../", import.meta.url);
const SKIP = [/(test|bench|\/_|\\_|testdata|version.ts)/];
const DECLARATION = "// This module is browser compatible.";

function isBrowserCompatible(filePath: string): boolean {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "check",
      "--config",
      "browser-compat.tsconfig.json",
      filePath,
    ],
  });
  const { success } = command.outputSync();
  return success;
}

function hasBrowserCompatibleComment(path: string): boolean {
  const output = Deno.readTextFileSync(path);
  return output.includes(DECLARATION);
}

const maybeBrowserCompatibleFiles: string[] = [];

for (const { path } of walkSync(ROOT, { exts: [".ts"], skip: SKIP })) {
  if (isBrowserCompatible(path) && !hasBrowserCompatibleComment(path)) {
    maybeBrowserCompatibleFiles.push(path);
  }
}

if (maybeBrowserCompatibleFiles.length) {
  console.log(
    `The following files are likely browser-compatible and can have the "${DECLARATION}" comment added:`,
  );
  maybeBrowserCompatibleFiles.forEach((path, index) =>
    console.log(`${index + 1}. ${path}`)
  );
}
