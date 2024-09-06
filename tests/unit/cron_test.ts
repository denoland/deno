// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "./test_util.ts";

// @ts-ignore This is not publicly typed namespace, but it's there for sure.
const {
  formatToCronSchedule,
  parseScheduleToString,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

Deno.test(function noNameTest() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron(),
    TypeError,
    "Cannot create cron job, a unique name is required: received 'undefined'",
  );
});

Deno.test(function noSchedule() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron("foo"),
    TypeError,
    "Cannot create cron job, a schedule is required: received 'undefined'",
  );
});

Deno.test(function noHandler() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron("foo", "*/1 * * * *"),
    TypeError,
    "Cannot create cron job: a handler is required",
  );
});

Deno.test(function invalidNameTest() {
  assertThrows(
    () => Deno.cron("abc[]", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("a**bc", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("abc<>", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron(";']", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () =>
      Deno.cron(
        "0000000000000000000000000000000000000000000000000000000000000000000000",
        "*/1 * * * *",
        () => {},
      ),
    TypeError,
    "Cron name cannot exceed 64 characters: current length 70",
  );
});

Deno.test(function invalidScheduleTest() {
  assertThrows(
    () => Deno.cron("abc", "bogus", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "* * * * * *", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "* * * *", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "m * * * *", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
});

Deno.test(function invalidBackoffScheduleTest() {
  assertThrows(
    () =>
      Deno.cron(
        "abc",
        "*/1 * * * *",
        { backoffSchedule: [1, 1, 1, 1, 1, 1] },
        () => {},
      ),
    TypeError,
    "Invalid backoff schedule",
  );
  assertThrows(
    () =>
      Deno.cron("abc", "*/1 * * * *", { backoffSchedule: [3600001] }, () => {}),
    TypeError,
    "Invalid backoff schedule",
  );
});

Deno.test(async function tooManyCrons() {
  const crons: Promise<void>[] = [];
  const ac = new AbortController();
  for (let i = 0; i <= 100; i++) {
    const c = Deno.cron(
      `abc_${i}`,
      "*/1 * * * *",
      { signal: ac.signal },
      () => {},
    );
    crons.push(c);
  }

  try {
    assertThrows(
      () => {
        Deno.cron("next-cron", "*/1 * * * *", { signal: ac.signal }, () => {});
      },
      TypeError,
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
  const c = Deno.cron("abc", "*/20 * * * *", { signal: ac.signal }, () => {});
  try {
    assertThrows(
      () => Deno.cron("abc", "*/20 * * * *", () => {}),
      TypeError,
      "Cron with this name already exists",
    );
  } finally {
    ac.abort();
    await c;
  }
});

Deno.test(async function basicTest() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count = 0;
  const { promise, resolve } = Promise.withResolvers<void>();
  const ac = new AbortController();
  const c = Deno.cron("abc", "*/20 * * * *", { signal: ac.signal }, () => {
    count++;
    if (count > 5) {
      resolve();
    }
  });
  try {
    await promise;
  } finally {
    ac.abort();
    await c;
  }
});

Deno.test(async function basicTestWithJsonFormatScheduleExpression() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count = 0;
  const { promise, resolve } = Promise.withResolvers<void>();
  const ac = new AbortController();
  const c = Deno.cron(
    "abc",
    { minute: { every: 20 } },
    { signal: ac.signal },
    () => {
      count++;
      if (count > 5) {
        resolve();
      }
    },
  );
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
  const { promise: promise0, resolve: resolve0 } = Promise.withResolvers<
    void
  >();
  const { promise: promise1, resolve: resolve1 } = Promise.withResolvers<
    void
  >();
  const ac = new AbortController();
  const c0 = Deno.cron("abc", "*/20 * * * *", { signal: ac.signal }, () => {
    count0++;
    if (count0 > 5) {
      resolve0();
    }
  });
  const c1 = Deno.cron("xyz", "*/20 * * * *", { signal: ac.signal }, () => {
    count1++;
    if (count1 > 5) {
      resolve1();
    }
  });
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
  const { promise: promise0, resolve: resolve0 } = Promise.withResolvers<
    void
  >();
  const { promise: promise1, resolve: resolve1 } = Promise.withResolvers<
    void
  >();
  const ac = new AbortController();
  const c = Deno.cron(
    "abc",
    "*/20 * * * *",
    { signal: ac.signal },
    async () => {
      resolve0();
      count++;
      await promise1;
    },
  );
  try {
    await promise0;
  } finally {
    await sleep(2000);
    resolve1();
    ac.abort();
    await c;
  }
  assertEquals(count, 1);
});

Deno.test(async function retriesWithBackoffSchedule() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "5000");

  let count = 0;
  const ac = new AbortController();
  const c = Deno.cron("abc", "*/20 * * * *", {
    signal: ac.signal,
    backoffSchedule: [10, 20],
  }, async () => {
    count += 1;
    await sleep(10);
    throw new TypeError("cron error");
  });
  try {
    await sleep(6000);
  } finally {
    ac.abort();
    await c;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count, 3);
});

Deno.test(async function retriesWithBackoffScheduleOldApi() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "5000");

  let count = 0;
  const ac = new AbortController();
  const c = Deno.cron("abc2", "*/20 * * * *", {
    signal: ac.signal,
    backoffSchedule: [10, 20],
  }, async () => {
    count += 1;
    await sleep(10);
    throw new TypeError("cron error");
  });

  try {
    await sleep(6000);
  } finally {
    ac.abort();
    await c;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count, 3);
});

Deno.test("formatToCronSchedule - undefined value", () => {
  const result = formatToCronSchedule();
  assertEquals(result, "*");
});

Deno.test("formatToCronSchedule - number value", () => {
  const result = formatToCronSchedule(5);
  assertEquals(result, "5");
});

Deno.test("formatToCronSchedule - exact array value", () => {
  const result = formatToCronSchedule({ exact: [1, 2, 3] });
  assertEquals(result, "1,2,3");
});

Deno.test("formatToCronSchedule - exact number value", () => {
  const result = formatToCronSchedule({ exact: 5 });
  assertEquals(result, "5");
});

Deno.test("formatToCronSchedule - start, end, every values", () => {
  const result = formatToCronSchedule({ start: 1, end: 10, every: 2 });
  assertEquals(result, "1-10/2");
});

Deno.test("formatToCronSchedule - start, end values", () => {
  const result = formatToCronSchedule({ start: 1, end: 10 });
  assertEquals(result, "1-10");
});

Deno.test("formatToCronSchedule - start, every values", () => {
  const result = formatToCronSchedule({ start: 1, every: 2 });
  assertEquals(result, "1/2");
});

Deno.test("formatToCronSchedule - start value", () => {
  const result = formatToCronSchedule({ start: 1 });
  assertEquals(result, "1/1");
});

Deno.test("formatToCronSchedule - end, every values", () => {
  assertThrows(
    () => formatToCronSchedule({ end: 10, every: 2 }),
    TypeError,
    "Invalid cron schedule",
  );
});

Deno.test("Parse CronSchedule to string", () => {
  const result = parseScheduleToString({
    minute: { exact: [1, 2, 3] },
    hour: { start: 1, end: 10, every: 2 },
    dayOfMonth: { exact: 5 },
    month: { start: 1, end: 10 },
    dayOfWeek: { start: 1, every: 2 },
  });
  assertEquals(result, "1,2,3 1-10/2 5 1-10 1/2");
});

Deno.test("Parse schedule to string - string", () => {
  const result = parseScheduleToString("* * * * *");
  assertEquals(result, "* * * * *");
});

Deno.test("error on two handlers", () => {
  assertThrows(
    () => {
      // @ts-ignore test
      Deno.cron("abc", "* * * * *", () => {}, () => {});
    },
    TypeError,
    "Cannot create cron job, a single handler is required: two handlers were specified",
  );
});

Deno.test("Parse test", () => {
  assertEquals(
    parseScheduleToString({
      minute: 3,
    }),
    "3 * * * *",
  );
  assertEquals(
    parseScheduleToString({
      hour: { every: 2 },
    }),
    "0 */2 * * *",
  );
  assertEquals(
    parseScheduleToString({
      dayOfMonth: { every: 10 },
    }),
    "0 0 */10 * *",
  );
  assertEquals(
    parseScheduleToString({
      month: { every: 3 },
    }),
    "0 0 1 */3 *",
  );
  assertEquals(
    parseScheduleToString({
      dayOfWeek: { every: 2 },
    }),
    "0 0 * * */2",
  );
  assertEquals(
    parseScheduleToString({
      minute: 3,
      hour: { every: 2 },
    }),
    "3 */2 * * *",
  );
  assertEquals(
    parseScheduleToString({
      dayOfMonth: { start: 1, end: 10 },
    }),
    "0 0 1-10 * *",
  );
  assertEquals(
    parseScheduleToString({
      minute: { every: 10 },
      dayOfMonth: { every: 5 },
    }),
    "*/10 * */5 * *",
  );
  assertEquals(
    parseScheduleToString({
      hour: { every: 3 },
      month: { every: 2 },
    }),
    "0 */3 * */2 *",
  );
  assertEquals(
    parseScheduleToString({
      minute: { every: 5 },
      month: { every: 2 },
    }),
    "*/5 * * */2 *",
  );
});
