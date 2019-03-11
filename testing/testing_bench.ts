import { bench, runIfMain } from "./bench.ts";
import { runTests } from "./mod.ts";

import "./asserts_test.ts";

bench(async function testingSerial(b) {
  b.start();
  await runTests();
  b.stop();
});

bench(async function testingParallel(b) {
  b.start();
  await runTests({ parallel: true });
  b.stop();
});

runIfMain(import.meta);
