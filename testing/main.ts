// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { runTests } from "./mod.ts";

async function main(): Promise<void> {
  // Testing entire test suite serially
  await runTests();
}

main();
