# benching

Basic benchmarking module. Provides flintstone millisecond resolution.

## Import

```ts
import * as benching from "https://deno.land/x/benching/mod.ts";
```

## Usage

```ts
import {
  BenchmarkTimer,
  runBenchmarks,
  bench
} from "https://deno.land/x/benching/mod.ts";

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
    for (let i: number = 0; i < 1e6; i++);
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
```

## API

#### `bench(benchmark: BenchmarkDefinition | BenchmarkFunction): void`

Registers a benchmark that will be run once `runBenchmarks` is called.

#### `runBenchmarks(opts?: BenchmarkRunOptions): Promise<void>`

Runs all registered benchmarks serially. Filtering can be applied by setting
`BenchmarkRunOptions.only` and/or `BenchmarkRunOptions.skip` to regular expressions matching benchmark names.

#### Other exports

```ts
/** Provides methods for starting and stopping a benchmark clock. */
export interface BenchmarkTimer {
  start: () => void;
  stop: () => void;
}

/** Defines a benchmark through a named function. */
export type BenchmarkFunction = {
  (b: BenchmarkTimer): void | Promise<void>;
  name: string;
};

/** Defines a benchmark definition with configurable runs. */
export interface BenchmarkDefinition {
  func: BenchmarkFunction;
  name: string;
  runs?: number;
}

/** Defines runBenchmark's run constraints by matching benchmark names. */
export interface BenchmarkRunOptions {
  only?: RegExp;
  skip?: RegExp;
}
```
