// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, deferred } from "./test_util.ts";

const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

Deno.test(function noNameTest() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron(),
    "Deno.cron requires a unique name",
  );
});

Deno.test(function noSchedule() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron("foo"),
    "Deno.cron requires a valid schedule",
  );
});

Deno.test(function noHandler() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron("foo", "*/1 * * * *"),
    "Deno.cron requires a valid schedule",
  );
});

Deno.test(function invalidNameTest() {
  assertThrows(
    () => Deno.cron("abc[]", "*/1 * * * *", () => {}),
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("a**bc", "*/1 * * * *", () => {}),
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("abc<>", "*/1 * * * *", () => {}),
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron(";']", "*/1 * * * *", () => {}),
    "Invalid cron name",
  );
  assertThrows(
    () =>
      Deno.cron(
        "0000000000000000000000000000000000000000000000000000000000000000000000",
        "*/1 * * * *",
        () => {},
      ),
    "Invalid cron name",
  );
});

Deno.test(function invalidScheduleTest() {
  assertThrows(
    () => Deno.cron("abc", "bogus", () => {}),
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "* * * * * *", () => {}),
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "* * * *", () => {}),
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "m * * * *", () => {}),
    "Invalid cron name",
  );
});

Deno.test(function invalidBackoffScheduleTest() {
  assertThrows(
    () =>
      Deno.cron("abc", "*/1 * * * *", () => {}, {
        backoffSchedule: [1, 1, 1, 1, 1, 1],
      }),
    "Invalid backoff schedule",
  );
  assertThrows(
    () =>
      Deno.cron("abc", "*/1 * * * *", () => {}, {
        backoffSchedule: [3600001],
      }),
    "Invalid backoff schedule",
  );
});

Deno.test(async function tooManyCrons() {
  const crons: Promise<void>[] = [];
  const ac = new AbortController();
  for (let i = 0; i <= 100; i++) {
    const c = Deno.cron(`abc_${i}`, "*/1 * * * *", () => {}, {
      signal: ac.signal,
    });
    crons.push(c);
  }

  try {
    assertThrows(
      () => {
        Deno.cron("next-cron", "*/1 * * * *", () => {}, { signal: ac.signal });
      },
      "Too many crons",
    );
  } finally {
    ac.abort();
    for (const c of crons) {
      await c;
    }
  }
});

Deno.test(async function duplicateCrons() {
  const ac = new AbortController();
  const c = Deno.cron("abc", "*/20 * * * *", () => {
  }, { signal: ac.signal });
  try {
    assertThrows(
      () => Deno.cron("abc", "*/20 * * * *", () => {}),
      "Invalid cron name",
    );
  } finally {
    ac.abort();
    await c;
  }
});

Deno.test(async function basicTest() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count = 0;
  const promise = deferred();
  const ac = new AbortController();
  const c = Deno.cron("abc", "*/20 * * * *", () => {
    count++;
    if (count > 5) {
      promise.resolve();
    }
  }, { signal: ac.signal });
  try {
    await promise;
  } finally {
    ac.abort();
    await c;
  }
});

Deno.test(async function multipleCrons() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count0 = 0;
  let count1 = 0;
  const promise0 = deferred();
  const promise1 = deferred();
  const ac = new AbortController();
  const c0 = Deno.cron("abc", "*/20 * * * *", () => {
    count0++;
    if (count0 > 5) {
      promise0.resolve();
    }
  }, { signal: ac.signal });
  const c1 = Deno.cron("xyz", "*/20 * * * *", () => {
    count1++;
    if (count1 > 5) {
      promise1.resolve();
    }
  }, { signal: ac.signal });
  try {
    await promise0;
    await promise1;
  } finally {
    ac.abort();
    await c0;
    await c1;
  }
});

Deno.test(async function overlappingExecutions() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count = 0;
  const promise0 = deferred();
  const promise1 = deferred();
  const ac = new AbortController();
  const c = Deno.cron("abc", "*/20 * * * *", async () => {
    promise0.resolve();
    count++;
    await promise1;
  }, { signal: ac.signal });
  try {
    await promise0;
  } finally {
    await sleep(2000);
    promise1.resolve();
    ac.abort();
    await c;
  }
  assertEquals(count, 1);
});

Deno.test(async function retriesWithBackkoffSchedule() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "5000");

  let count = 0;
  const ac = new AbortController();
  const c = Deno.cron("abc", "*/20 * * * *", async () => {
    count += 1;
    await sleep(10);
    throw new TypeError("cron error");
  }, { signal: ac.signal, backoffSchedule: [10, 20] });
  try {
    await sleep(6000);
  } finally {
    ac.abort();
    await c;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count, 3);
});
