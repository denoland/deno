import { bench, runBenchmarks } from "./../benching/mod.ts";
import { runTests } from "./mod.ts";

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

runBenchmarks({ only: /testing/ });
