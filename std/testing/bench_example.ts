// https://deno.land/std/testing/bench.ts
import { BenchmarkTimer, bench, runBenchmarks } from "./bench.ts";

// Basic
bench(function forIncrementX1e9(b: BenchmarkTimer): void {
  b.start();
  for (let i = 0; i < 1e9; i++);
  b.stop();
});

// Reporting average measured time for $runs runs of func
bench({
  name: "runs100ForIncrementX1e6",
  runs: 100,
  func(b): void {
    b.start();
    for (let i = 0; i < 1e6; i++);
    b.stop();
  },
});

// Itsabug
bench(function throwing(b): void {
  b.start();
  // Throws bc the timer's stop method is never called
});

// Bench control
if (import.meta.main) {
  runBenchmarks({ skip: /throw/ });
}
