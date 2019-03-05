// https://deno.land/x/benching/mod.ts
import { BenchmarkTimer, runBenchmarks, bench } from "./mod.ts";

// Simple
bench(function forIncrementX1e9(b: BenchmarkTimer) {
  b.start();
  for (let i = 0; i < 1e9; i++);
  b.stop();
});

// Reporting average measured time for $runs runs of func
bench({
  name: "runs100ForIncrementX1e6",
  runs: 100,
  func(b: BenchmarkTimer) {
    b.start();
    for (let i = 0; i < 1e6; i++);
    b.stop();
  }
});

// Itsabug
bench(function throwing(b) {
  b.start();
  // Throws bc the timer's stop method is never called
});

// Bench control
runBenchmarks({ skip: /throw/ });
