// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { runTests } from "./mod.ts";

async function main() {
  // Testing entire test suite serially
  await runTests();
  // Testing parallel execution on a subset that does not depend on exec order
  await runTests({ parallel: true, only: /^testing/ });
}

main();
