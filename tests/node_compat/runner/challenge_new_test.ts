// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-console

import { deadline } from "@std/async/deadline";
import { ensureDir } from "@std/fs/ensure-dir";
import { copy } from "@std/fs/copy";
import { withoutAll } from "@std/collections/without-all";
import {
  getDenoTests,
  getNodeTests,
  NODE_COMPAT_TEST_DEST_URL,
  runNodeCompatTestCase,
  VENDORED_NODE_TEST,
} from "../common.ts";
import { fromFileUrl } from "@std/path/from-file-url";

/** The timeout ms for single test execution. If a single test didn't finish in this timeout milliseconds, the test is considered as failure */
const TIMEOUT = 2000;

async function main() {
  const remainingTests = withoutAll(await getNodeTests(), await getDenoTests());

  console.log(`Remaining tests: ${remainingTests.length}`);
  const success = [] as string[];
  let i = 0;

  Deno.addSignalListener("SIGINT", () => {
    console.log(`Success: ${success.length}`);
    for (const testPath of success) {
      console.log(testPath);
    }
    Deno.exit(1);
  });

  for (const testPath of remainingTests) {
    i++;
    const source = new URL(testPath, VENDORED_NODE_TEST);
    const dest = new URL(testPath, NODE_COMPAT_TEST_DEST_URL);

    await ensureDir(new URL(".", dest));
    await copy(source, dest);
    const num = String(i).padStart(4, " ");
    try {
      const cp = await runNodeCompatTestCase(
        fromFileUrl(dest),
        AbortSignal.timeout(TIMEOUT),
      );
      const result = await deadline(cp.output(), TIMEOUT + 1000);
      if (result.code === 0) {
        console.log(`${num} %cPASS`, "color: green", testPath);
        success.push(testPath);
      } else {
        console.log(`${num} %cFAIL`, "color: red", testPath);
      }
    } catch (e) {
      if (e instanceof DOMException && e.name === "TimeoutError") {
        console.log(`${num} %cFAIL`, "color: red", testPath);
      } else {
        console.log(`Unexpected Error`, e);
      }
    } finally {
      await Deno.remove(dest);
    }
  }
  console.log(`Success: ${success.length}`);
  for (const testPath of success) {
    console.log(testPath);
  }
  Deno.exit(0);
}

await main();
