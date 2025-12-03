// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertRejects, fail } from "@std/assert";
import * as timers from "node:timers";
import * as timersPromises from "node:timers/promises";
import { assertEquals } from "@std/assert";
import { performance } from "node:perf_hooks";

Deno.test("[node/perf_hooks] performance.timerify()", () => {
  function sayHello() {
    return "hello world";
  }

  const wrapped = performance.timerify(sayHello);
  const result = wrapped();

  if (result !== "hello world") {
    throw new Error(`Expected "hello world", got "${result}"`);
  }
});

Deno.test("[node/timers setTimeout]", () => {
  {
    const { clearTimeout, setTimeout } = timers;
    const id = setTimeout(() => {});
    clearTimeout(id);
  }

  {
    const id = timers.setTimeout(() => {});
    timers.clearTimeout(id);
  }
});

Deno.test("[node/timers setInterval]", () => {
  {
    const { clearInterval, setInterval } = timers;
    const id = setInterval(() => {});
    clearInterval(id);
  }

  {
    const id = timers.setInterval(() => {});
    timers.clearInterval(id);
  }
});

Deno.test("[node/timers setImmediate]", async () => {
  {
    const { clearImmediate, setImmediate } = timers;
    const imm = setImmediate(() => {});
    clearImmediate(imm);
  }

  {
    const imm = timers.setImmediate(() => {});
    timers.clearImmediate(imm);
  }

  {
    const deffered = Promise.withResolvers<void>();
    const imm = timers.setImmediate(
      (a, b) => {
        assert(a === 1);
        assert(b === 2);
        deffered.resolve();
      },
      1,
      2,
    );
    await deffered;
    timers.clearImmediate(imm);
  }
});

Deno.test("[node/timers/promises setTimeout]", () => {
  const { setTimeout } = timersPromises;
  const p = setTimeout(0);

  assert(p instanceof Promise);
  return p;
});

Deno.test("[node/timers/promises scheduler.wait]", async () => {
  const { scheduler } = timersPromises;
  let resolved = false;
  timers.setTimeout(() => (resolved = true), 20);
  const p = scheduler.wait(20);

  assert(p instanceof Promise);
  await p;
  assert(resolved);
});

Deno.test("[node/timers/promises scheduler.yield]", async () => {
  const { scheduler } = timersPromises;
  let resolved = false;
  timers.setImmediate(() => resolved = true);

  const p = scheduler.yield();
  assert(p instanceof Promise);
  await p;

  assert(resolved);
});

// Regression test for https://github.com/denoland/deno/issues/17981
Deno.test("[node/timers refresh cancelled timer]", () => {
  const { setTimeout, clearTimeout } = timers;
  const p = setTimeout(() => {
    fail();
  }, 1);
  clearTimeout(p);
  p.refresh();
});

Deno.test("[node/timers] clearTimeout with number", () => {
  const timer = +timers.setTimeout(() => fail(), 10);
  timers.clearTimeout(timer);
});

Deno.test("[node/timers] clearInterval with number", () => {
  const timer = +timers.setInterval(() => fail(), 10);
  timers.clearInterval(timer);
});

Deno.test("[node/timers setImmediate returns Immediate object]", () => {
  const { clearImmediate, setImmediate } = timers;

  const imm = setImmediate(() => {});
  imm.unref();
  imm.ref();
  imm.hasRef();
  clearImmediate(imm);
});

Deno.test({
  name: "setInterval yields correct values at expected intervals",
  async fn() {
    // Test configuration
    const CONFIG = {
      expectedValue: 42,
      intervalMs: 100,
      iterations: 3,
      tolerancePercent: Deno.env.get("CI") != null ? 75 : 50,
    };

    const { setInterval } = timersPromises;
    const results: Array<{ value: number; timestamp: number }> = [];
    const startTime = Date.now();

    const iterator = setInterval(CONFIG.intervalMs, CONFIG.expectedValue);

    for await (const value of iterator) {
      results.push({
        value,
        timestamp: Date.now(),
      });
      if (results.length === CONFIG.iterations) {
        break;
      }
    }

    const values = results.map((r) => r.value);
    assertEquals(
      values,
      Array(CONFIG.iterations).fill(CONFIG.expectedValue),
      `Each iteration should yield ${CONFIG.expectedValue}`,
    );

    const intervals = results.slice(1).map((result, index) => ({
      interval: result.timestamp - results[index].timestamp,
      iterationNumber: index + 1,
    }));

    const toleranceMs = (CONFIG.tolerancePercent / 100) * CONFIG.intervalMs;
    const expectedRange = {
      min: CONFIG.intervalMs - toleranceMs,
      max: CONFIG.intervalMs + toleranceMs,
    };

    intervals.forEach(({ interval, iterationNumber }) => {
      const isWithinTolerance = interval >= expectedRange.min &&
        interval <= expectedRange.max;

      assertEquals(
        isWithinTolerance,
        true,
        `Iteration ${iterationNumber}: Interval ${interval}ms should be within ` +
          `${expectedRange.min}ms and ${expectedRange.max}ms ` +
          `(${CONFIG.tolerancePercent}% tolerance of ${CONFIG.intervalMs}ms)`,
      );
    });

    const totalDuration = results[results.length - 1].timestamp - startTime;
    const expectedDuration = CONFIG.intervalMs * CONFIG.iterations;
    const isDurationReasonable =
      totalDuration >= (expectedDuration - toleranceMs) &&
      totalDuration <= (expectedDuration + toleranceMs);

    assertEquals(
      isDurationReasonable,
      true,
      `Total duration ${totalDuration}ms should be close to ${expectedDuration}ms ` +
        `(within ${toleranceMs}ms tolerance)`,
    );

    const timestamps = results.map((r) => r.timestamp);
    const areTimestampsOrdered = timestamps.every((timestamp, i) =>
      i === 0 || timestamp > timestamps[i - 1]
    );

    assertEquals(
      areTimestampsOrdered,
      true,
      "Timestamps should be strictly increasing",
    );
  },
});

Deno.test({
  name: "setInterval with AbortSignal stops after expected duration",
  async fn() {
    const INTERVAL_MS = 500;
    const TOTAL_DURATION_MS = 3000;
    const TOLERANCE_MS = 500;
    const DELTA_TOLERANCE_MS = Deno.env.get("CI") != null ? 100 : 50;

    const abortController = new AbortController();
    const { setInterval } = timersPromises;

    // Set up abort after specified duration
    const abortTimeout = timers.setTimeout(() => {
      abortController.abort();
    }, TOTAL_DURATION_MS);

    // Track iterations and timing
    const startTime = Date.now();
    const iterations: number[] = [];

    try {
      for await (
        const _timestamp of setInterval(INTERVAL_MS, undefined, {
          signal: abortController.signal,
        })
      ) {
        iterations.push(Date.now() - startTime);
      }
    } catch (error) {
      if (error instanceof Error && error.name !== "AbortError") {
        throw error;
      }
    } finally {
      timers.clearTimeout(abortTimeout);
    }

    // Validate timing
    const totalDuration = iterations[iterations.length - 1];
    const isWithinTolerance =
      totalDuration >= (TOTAL_DURATION_MS - TOLERANCE_MS) &&
      totalDuration <= (TOTAL_DURATION_MS + TOLERANCE_MS);

    assertEquals(
      isWithinTolerance,
      true,
      `Total duration ${totalDuration}ms should be within ±${TOLERANCE_MS}ms of ${TOTAL_DURATION_MS}ms`,
    );

    // Validate interval consistency
    const intervalDeltas = iterations.slice(1).map((time, i) =>
      time - iterations[i]
    );

    intervalDeltas.forEach((delta, i) => {
      const isIntervalValid = delta >= (INTERVAL_MS - DELTA_TOLERANCE_MS) &&
        delta <= (INTERVAL_MS + DELTA_TOLERANCE_MS);
      assertEquals(
        isIntervalValid,
        true,
        `Interval ${
          i + 1
        } duration (${delta}ms) should be within ±${DELTA_TOLERANCE_MS}ms of ${INTERVAL_MS}ms`,
      );
    });
  },
});

Deno.test({
  name: "[timers/promises] setTimeout aborted by AbortSignal",
  async fn() {
    const timerPromise = new Promise((resolve, reject) => {
      const abortController = new AbortController();
      const timer = timersPromises.setTimeout(1000, "foo", {
        signal: abortController.signal,
      });
      abortController.abort();
      timer.then(resolve).catch(reject);
    });

    await assertRejects(
      () => timerPromise,
      Error,
      "The operation was aborted",
    );
  },
});
