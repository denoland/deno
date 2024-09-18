// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

import { core, primordials } from "ext:core/mod.js";
import {
  escapeName,
  pledgePermissions,
  restorePermissions,
} from "ext:cli/40_test_common.js";
import { Console } from "ext:deno_console/01_console.js";
import { setExitHandler } from "ext:runtime/30_os.js";
const {
  op_register_bench,
  op_bench_get_origin,
  op_dispatch_bench_event,
  op_bench_now,
} = core.ops;
const {
  ArrayPrototypePush,
  Error,
  MathCeil,
  SymbolToStringTag,
  TypeError,
} = primordials;

/** @type {number | null} */
let currentBenchId = null;
// These local variables are used to track time measurements at
// `BenchContext::{start,end}` calls. They are global instead of using a state
// map to minimise the overhead of assigning them.
/** @type {number | null} */
let currentBenchUserExplicitStart = null;
/** @type {number | null} */
let currentBenchUserExplicitEnd = null;

let registeredWarmupBench = false;

const registerBenchIdRetBuf = new Uint32Array(1);
const registerBenchIdRetBufU8 = new Uint8Array(registerBenchIdRetBuf.buffer);

// As long as we're using one isolate per test, we can cache the origin since it won't change
let cachedOrigin = undefined;

// Main bench function provided by Deno.
function bench(
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
) {
  // No-op if we're not running in `deno bench` subcommand.
  if (typeof op_register_bench !== "function") {
    return;
  }

  if (!registeredWarmupBench) {
    registeredWarmupBench = true;
    const warmupBenchDesc = {
      name: "<warmup>",
      fn: function warmup() {},
      async: false,
      ignore: false,
      baseline: false,
      only: false,
      sanitizeExit: true,
      permissions: null,
      warmup: true,
    };
    if (cachedOrigin == undefined) {
      cachedOrigin = op_bench_get_origin();
    }
    warmupBenchDesc.fn = wrapBenchmark(warmupBenchDesc);
    op_register_bench(
      warmupBenchDesc.fn,
      warmupBenchDesc.name,
      warmupBenchDesc.baseline,
      warmupBenchDesc.group,
      warmupBenchDesc.ignore,
      warmupBenchDesc.only,
      warmupBenchDesc.warmup,
      registerBenchIdRetBufU8,
    );
    warmupBenchDesc.id = registerBenchIdRetBufU8[0];
    warmupBenchDesc.origin = cachedOrigin;
  }

  let benchDesc;
  const defaults = {
    ignore: false,
    baseline: false,
    only: false,
    sanitizeExit: true,
    permissions: null,
  };

  if (typeof nameOrFnOrOptions === "string") {
    if (!nameOrFnOrOptions) {
      throw new TypeError("The bench name can't be empty");
    }
    if (typeof optionsOrFn === "function") {
      benchDesc = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
    } else {
      if (!maybeFn || typeof maybeFn !== "function") {
        throw new TypeError("Missing bench function");
      }
      if (optionsOrFn.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, bench function is already provided as the third argument",
        );
      }
      if (optionsOrFn.name != undefined) {
        throw new TypeError(
          "Unexpected 'name' field in options, bench name is already provided as the first argument",
        );
      }
      benchDesc = {
        ...defaults,
        ...optionsOrFn,
        fn: maybeFn,
        name: nameOrFnOrOptions,
      };
    }
  } else if (typeof nameOrFnOrOptions === "function") {
    if (!nameOrFnOrOptions.name) {
      throw new TypeError("The bench function must have a name");
    }
    if (optionsOrFn != undefined) {
      throw new TypeError("Unexpected second argument to Deno.bench()");
    }
    if (maybeFn != undefined) {
      throw new TypeError("Unexpected third argument to Deno.bench()");
    }
    benchDesc = {
      ...defaults,
      fn: nameOrFnOrOptions,
      name: nameOrFnOrOptions.name,
    };
  } else {
    let fn;
    let name;
    if (typeof optionsOrFn === "function") {
      fn = optionsOrFn;
      if (nameOrFnOrOptions.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, bench function is already provided as the second argument",
        );
      }
      name = nameOrFnOrOptions.name ?? fn.name;
    } else {
      if (
        !nameOrFnOrOptions.fn || typeof nameOrFnOrOptions.fn !== "function"
      ) {
        throw new TypeError(
          "Expected 'fn' field in the first argument to be a bench function",
        );
      }
      fn = nameOrFnOrOptions.fn;
      name = nameOrFnOrOptions.name ?? fn.name;
    }
    if (!name) {
      throw new TypeError("The bench name can't be empty");
    }
    benchDesc = { ...defaults, ...nameOrFnOrOptions, fn, name };
  }

  const AsyncFunction = (async () => {}).constructor;
  benchDesc.async = AsyncFunction === benchDesc.fn.constructor;
  benchDesc.fn = wrapBenchmark(benchDesc);
  benchDesc.warmup = false;
  benchDesc.name = escapeName(benchDesc.name);
  if (cachedOrigin == undefined) {
    cachedOrigin = op_bench_get_origin();
  }
  op_register_bench(
    benchDesc.fn,
    benchDesc.name,
    benchDesc.baseline,
    benchDesc.group,
    benchDesc.ignore,
    benchDesc.only,
    false,
    registerBenchIdRetBufU8,
  );
  benchDesc.id = registerBenchIdRetBufU8[0];
  benchDesc.origin = cachedOrigin;
}

function compareMeasurements(a, b) {
  if (a > b) return 1;
  if (a < b) return -1;

  return 0;
}

function benchStats(
  n,
  highPrecision,
  usedExplicitTimers,
  avg,
  min,
  max,
  all,
) {
  return {
    n,
    min,
    max,
    p75: all[MathCeil(n * (75 / 100)) - 1],
    p99: all[MathCeil(n * (99 / 100)) - 1],
    p995: all[MathCeil(n * (99.5 / 100)) - 1],
    p999: all[MathCeil(n * (99.9 / 100)) - 1],
    avg: !highPrecision ? (avg / n) : MathCeil(avg / n),
    highPrecision,
    usedExplicitTimers,
  };
}

async function benchMeasure(timeBudget, fn, async, context) {
  let n = 0;
  let avg = 0;
  let wavg = 0;
  let usedExplicitTimers = false;
  const all = [];
  let min = Infinity;
  let max = -Infinity;
  const lowPrecisionThresholdInNs = 1e4;

  // warmup step
  let c = 0;
  let iterations = 20;
  let budget = 10 * 1e6;

  if (!async) {
    while (budget > 0 || iterations-- > 0) {
      const t1 = benchNow();
      fn(context);
      const t2 = benchNow();
      const totalTime = t2 - t1;
      if (currentBenchUserExplicitStart !== null) {
        currentBenchUserExplicitStart = null;
        usedExplicitTimers = true;
      }
      if (currentBenchUserExplicitEnd !== null) {
        currentBenchUserExplicitEnd = null;
        usedExplicitTimers = true;
      }

      c++;
      wavg += totalTime;
      budget -= totalTime;
    }
  } else {
    while (budget > 0 || iterations-- > 0) {
      const t1 = benchNow();
      await fn(context);
      const t2 = benchNow();
      const totalTime = t2 - t1;
      if (currentBenchUserExplicitStart !== null) {
        currentBenchUserExplicitStart = null;
        usedExplicitTimers = true;
      }
      if (currentBenchUserExplicitEnd !== null) {
        currentBenchUserExplicitEnd = null;
        usedExplicitTimers = true;
      }

      c++;
      wavg += totalTime;
      budget -= totalTime;
    }
  }

  wavg /= c;

  // measure step
  if (wavg > lowPrecisionThresholdInNs) {
    let iterations = 10;
    let budget = timeBudget * 1e6;

    if (!async) {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();
        fn(context);
        const t2 = benchNow();
        const totalTime = t2 - t1;
        let measuredTime = totalTime;
        if (currentBenchUserExplicitStart !== null) {
          measuredTime -= currentBenchUserExplicitStart - t1;
          currentBenchUserExplicitStart = null;
        }
        if (currentBenchUserExplicitEnd !== null) {
          measuredTime -= t2 - currentBenchUserExplicitEnd;
          currentBenchUserExplicitEnd = null;
        }

        n++;
        avg += measuredTime;
        budget -= totalTime;
        ArrayPrototypePush(all, measuredTime);
        if (measuredTime < min) min = measuredTime;
        if (measuredTime > max) max = measuredTime;
      }
    } else {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();
        await fn(context);
        const t2 = benchNow();
        const totalTime = t2 - t1;
        let measuredTime = totalTime;
        if (currentBenchUserExplicitStart !== null) {
          measuredTime -= currentBenchUserExplicitStart - t1;
          currentBenchUserExplicitStart = null;
        }
        if (currentBenchUserExplicitEnd !== null) {
          measuredTime -= t2 - currentBenchUserExplicitEnd;
          currentBenchUserExplicitEnd = null;
        }

        n++;
        avg += measuredTime;
        budget -= totalTime;
        ArrayPrototypePush(all, measuredTime);
        if (measuredTime < min) min = measuredTime;
        if (measuredTime > max) max = measuredTime;
      }
    }
  } else {
    context.start = function start() {};
    context.end = function end() {};
    let iterations = 10;
    let budget = timeBudget * 1e6;

    if (!async) {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();
        for (let c = 0; c < lowPrecisionThresholdInNs; c++) {
          fn(context);
        }
        const iterationTime = (benchNow() - t1) / lowPrecisionThresholdInNs;

        n++;
        avg += iterationTime;
        ArrayPrototypePush(all, iterationTime);
        if (iterationTime < min) min = iterationTime;
        if (iterationTime > max) max = iterationTime;
        budget -= iterationTime * lowPrecisionThresholdInNs;
      }
    } else {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();
        for (let c = 0; c < lowPrecisionThresholdInNs; c++) {
          await fn(context);
          currentBenchUserExplicitStart = null;
          currentBenchUserExplicitEnd = null;
        }
        const iterationTime = (benchNow() - t1) / lowPrecisionThresholdInNs;

        n++;
        avg += iterationTime;
        ArrayPrototypePush(all, iterationTime);
        if (iterationTime < min) min = iterationTime;
        if (iterationTime > max) max = iterationTime;
        budget -= iterationTime * lowPrecisionThresholdInNs;
      }
    }
  }

  all.sort(compareMeasurements);
  return benchStats(
    n,
    wavg > lowPrecisionThresholdInNs,
    usedExplicitTimers,
    avg,
    min,
    max,
    all,
  );
}

/** @param desc {BenchDescription} */
function createBenchContext(desc) {
  return {
    [SymbolToStringTag]: "BenchContext",
    name: desc.name,
    origin: desc.origin,
    start() {
      if (currentBenchId !== desc.id) {
        throw new TypeError(
          "The benchmark which this context belongs to is not being executed",
        );
      }
      if (currentBenchUserExplicitStart != null) {
        throw new TypeError(
          "BenchContext::start() has already been invoked",
        );
      }
      currentBenchUserExplicitStart = benchNow();
    },
    end() {
      const end = benchNow();
      if (currentBenchId !== desc.id) {
        throw new TypeError(
          "The benchmark which this context belongs to is not being executed",
        );
      }
      if (currentBenchUserExplicitEnd != null) {
        throw new TypeError("BenchContext::end() has already been invoked");
      }
      currentBenchUserExplicitEnd = end;
    },
  };
}

/** Wrap a user benchmark function in one which returns a structured result. */
function wrapBenchmark(desc) {
  const fn = desc.fn;
  return async function outerWrapped() {
    let token = null;
    const originalConsole = globalThis.console;
    currentBenchId = desc.id;

    try {
      globalThis.console = new Console((s) => {
        op_dispatch_bench_event({ output: s });
      });

      if (desc.permissions) {
        token = pledgePermissions(desc.permissions);
      }

      if (desc.sanitizeExit) {
        setExitHandler((exitCode) => {
          throw new Error(
            `Bench attempted to exit with exit code: ${exitCode}`,
          );
        });
      }

      const benchTimeInMs = 500;
      const context = createBenchContext(desc);
      const stats = await benchMeasure(
        benchTimeInMs,
        fn,
        desc.async,
        context,
      );

      return { ok: stats };
    } catch (error) {
      return { failed: core.destructureError(error) };
    } finally {
      globalThis.console = originalConsole;
      currentBenchId = null;
      currentBenchUserExplicitStart = null;
      currentBenchUserExplicitEnd = null;
      if (bench.sanitizeExit) setExitHandler(null);
      if (token !== null) restorePermissions(token);
    }
  };
}

function benchNow() {
  return op_bench_now();
}

globalThis.Deno.bench = bench;
