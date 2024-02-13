// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { retry, RetryError } from "./retry.ts";
import { _exponentialBackoffWithJitter } from "./_util.ts";
import { assertEquals, assertRejects } from "../assert/mod.ts";
import { FakeTime } from "../testing/time.ts";

function generateErroringFunction(errorsBeforeSucceeds: number) {
  let errorCount = 0;

  return () => {
    if (errorCount >= errorsBeforeSucceeds) {
      return errorCount;
    }
    errorCount++;
    throw `Only errored ${errorCount} times`;
  };
}

Deno.test("[async] retry", async function () {
  const threeErrors = generateErroringFunction(3);
  const result = await retry(threeErrors, {
    minTimeout: 100,
  });
  assertEquals(result, 3);
});

Deno.test("[async] retry fails after max errors is passed", async function () {
  const fiveErrors = generateErroringFunction(5);
  await assertRejects(() =>
    retry(fiveErrors, {
      minTimeout: 100,
    })
  );
});

Deno.test("[async] retry waits four times by default", async function () {
  let callCount = 0;
  const onlyErrors = function () {
    callCount++;
    throw new Error("Failure");
  };
  const time = new FakeTime();
  const callCounts: Array<number> = [];
  try {
    const promise = retry(onlyErrors);
    queueMicrotask(() => callCounts.push(callCount));
    await time.next();
    queueMicrotask(() => callCounts.push(callCount));
    await time.next();
    queueMicrotask(() => callCounts.push(callCount));
    await time.next();
    queueMicrotask(() => callCounts.push(callCount));
    await time.next();
    queueMicrotask(() => callCounts.push(callCount));
    await assertRejects(() => promise, RetryError);
    assertEquals(callCounts, [1, 2, 3, 4, 5]);
  } finally {
    time.restore();
  }
});

Deno.test(
  "[async] retry throws if minTimeout is less than maxTimeout",
  async function () {
    await assertRejects(() =>
      retry(() => {}, {
        minTimeout: 1000,
        maxTimeout: 100,
      })
    );
  },
);

Deno.test(
  "[async] retry throws if maxTimeout is less than 0",
  async function () {
    await assertRejects(() =>
      retry(() => {}, {
        maxTimeout: -1,
      })
    );
  },
);

Deno.test(
  "[async] retry throws if jitter is bigger than 1",
  async function () {
    await assertRejects(() =>
      retry(() => {}, {
        jitter: 2,
      })
    );
  },
);

// test util to ensure deterministic results during testing of backoff function by polyfilling Math.random
function prngMulberry32(seed: number) {
  return function () {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0);
  };
}

// random seed generated with crypto.getRandomValues(new Uint32Array(1))[0]
const INITIAL_SEED = 3460544849;

const expectedTimings: readonly (readonly number[] & { length: 10 })[] & {
  length: 10;
} = [
  [31, 117, 344, 9, 1469, 1060, 920, 5094, 19564, 33292],
  [46, 184, 377, 419, 1455, 483, 3205, 8426, 22451, 29810],
  [68, 17, 66, 645, 1209, 246, 3510, 4598, 398, 12813],
  [46, 111, 374, 626, 859, 1955, 5379, 609, 5766, 33641],
  [26, 129, 287, 757, 1104, 4, 2557, 4940, 16657, 6888],
  [80, 71, 348, 245, 743, 128, 2445, 5722, 19960, 49861],
  [25, 46, 341, 498, 602, 2349, 1366, 4399, 1680, 9275],
  [14, 174, 189, 309, 1461, 937, 1898, 2087, 9624, 18872],
  [65, 190, 382, 351, 826, 2502, 5657, 3967, 1063, 43754],
  [89, 78, 222, 668, 1027, 1397, 1293, 8295, 14077, 33602],
] as const;

Deno.test("[async] retry - backoff function timings", async (t) => {
  const originalMathRandom = Math.random;

  await t.step("wait fixed times without jitter", async function () {
    const time = new FakeTime();
    let resolved = false;
    const checkResolved = async () => {
      try {
        await retry(() => {
          throw new Error("Failure");
        }, { jitter: 0 });
      } catch {
        resolved = true;
      }
    };
    try {
      const promise = checkResolved();
      const startTime = time.now;

      await time.nextAsync();
      assertEquals(time.now - startTime, 1000);

      await time.nextAsync();
      assertEquals(time.now - startTime, 3000);

      await time.nextAsync();
      assertEquals(time.now - startTime, 7000);

      await time.nextAsync();
      assertEquals(time.now - startTime, 15000);
      assertEquals(resolved, false);

      await time.runMicrotasks();
      assertEquals(time.now - startTime, 15000);
      assertEquals(resolved, true);

      await time.runAllAsync();
      assertEquals(time.now - startTime, 15000);
      await promise;
    } finally {
      time.restore();
    }
  });

  await t.step("_exponentialBackoffWithJitter", () => {
    let nextSeed = INITIAL_SEED;

    for (const row of expectedTimings) {
      const randUint32 = prngMulberry32(nextSeed);
      nextSeed = prngMulberry32(nextSeed)();
      Math.random = () => randUint32() / 0x100000000;

      const results: number[] = [];
      const base = 100;
      const cap = Infinity;

      for (let i = 0; i < 10; ++i) {
        const result = _exponentialBackoffWithJitter(cap, base, i, 2, 1);
        results.push(Math.round(result));
      }

      assertEquals(results as typeof row, row);
    }
  });

  Math.random = originalMathRandom;
});
