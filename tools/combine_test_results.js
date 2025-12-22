// Copyright 2018-2025 the Deno authors. MIT license.

import { join } from "@std/path";
import { ROOT_PATH } from "./util.js";

const joinTarget = (fileName) => join(ROOT_PATH, "target", fileName);
const filePaths = [
  "test_results_integration.json",
  "test_results_specs.json",
  "test_results_unit.json",
  "test_results_unit_node.json",
].map((fileName) => joinTarget(fileName));

const tests = [];
for (const filePath of filePaths) {
  try {
    tests.push(...JSON.parse(Deno.readTextFileSync(filePath)).tests);
  } catch (err) {
    if (!(err instanceof Deno.errors.NotFound)) {
      throw err;
    }
  }
}

const combinedFileText = JSON.stringify({ tests });
Deno.writeTextFileSync(joinTarget("test_results.json"), combinedFileText);
