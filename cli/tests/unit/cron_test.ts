// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "./test_util.ts";
import { formatToCronSchedule } from "../../../ext/cron/01_cron.ts";

const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

Deno.test(function noNameTest() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron(),
    TypeError,
    "Deno.cron requires a unique name",
  );
});

Deno.test(function noSchedule() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron("foo"),
    TypeError,
    "Deno.cron requires a valid schedule",
  );
});

Deno.test(function noHandler() {
  assertThrows(
    // @ts-ignore test
    () => Deno.cron("foo", "*/1 * * * *"),
    TypeError,
    "Deno.cron requires a handler",
  );
});

Deno.test(function invalidNameTest() {
  assertThrows(
    () => Deno.cron("abc[]", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("abc[]", { minute: 0 }, () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("a**bc", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("a**bc", { minute: 0 }, () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("abc<>", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron("abc<>", { minute: 0 }, () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron(";']", "*/1 * * * *", () => {}),
    TypeError,
    "Invalid cron name",
  );
  assertThrows(
    () => Deno.cron(";']", { minute: 0 }, () => {}),
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
    "Cron name is too long",
  );
  assertThrows(
    () =>
      Deno.cron(
        "0000000000000000000000000000000000000000000000000000000000000000000000",
        { minute: 0 },
        () => {},
      ),
    TypeError,
    "Cron name is too long",
  );
});

Deno.test(function invalidScheduleTest() {
  assertThrows(
    () => Deno.cron("abc", "bogus", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", { minute: { end: 10, every: 2 } }, () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "* * * * * *", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", { minute: 80 }, () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("abc", "* * * *", () => {}),
    TypeError,
    "Invalid cron schedule",
  );
  assertThrows(
    () => Deno.cron("fgh", { dayOfWeek: 90 }, () => {}),
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
      Deno.cron(
        "abc",
        { minute: 0 },
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
  const crons1: Promise<void>[] = [];
  const ac1 = new AbortController();
  for (let i = 0; i <= 100; i++) {
    const c = Deno.cron(
      `abc_${i}`,
      "*/1 * * * *",
      { signal: ac1.signal },
      () => {},
    );
    crons1.push(c);
  }

  try {
    assertThrows(
      () => {
        Deno.cron("next-cron", "*/1 * * * *", { signal: ac1.signal }, () => {});
      },
      TypeError,
      "Too many crons",
    );
  } finally {
    ac1.abort();
    for (const c of crons1) {
      await c;
    }
  }

  const crons2: Promise<void>[] = [];
  const ac2 = new AbortController();
  for (let i = 0; i <= 100; i++) {
    const c = Deno.cron(
      `abc_${i}`,
      { minute: { start: 0, every: 1 } },
      { signal: ac2.signal },
      () => {},
    );
    crons2.push(c);
  }

  try {
    assertThrows(
      () => {
        Deno.cron("next-cron", { minute: { start: 0, every: 1 } }, {
          signal: ac2.signal,
        }, () => {});
      },
      TypeError,
      "Too many crons",
    );
  } finally {
    ac2.abort();
    for (const c of crons2) {
      await c;
    }
  }
});

Deno.test(async function duplicateCrons() {
  const ac1 = new AbortController();
  const c1 = Deno.cron("abc", "*/20 * * * *", { signal: ac1.signal }, () => {});
  try {
    assertThrows(
      () => Deno.cron("abc", "*/20 * * * *", () => {}),
      TypeError,
      "Cron with this name already exists",
    );
  } finally {
    ac1.abort();
    await c1;
  }

  const ac2 = new AbortController();
  const c2 = Deno.cron("abc", "*/20 * * * *", { signal: ac2.signal }, () => {});
  try {
    assertThrows(
      () => Deno.cron("abc", { minute: { start: 0, every: 20 } }, () => {}),
      TypeError,
      "Cron with this name already exists",
    );
  } finally {
    ac2.abort();
    await c2;
  }
});

Deno.test(async function basicTest() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count1 = 0;
  const { promise, resolve } = Promise.withResolvers<void>();
  const ac1 = new AbortController();
  const c1 = Deno.cron("abc", "*/20 * * * *", { signal: ac1.signal }, () => {
    count1++;
    if (count1 > 5) {
      resolve();
    }
  });
  try {
    await promise;
  } finally {
    ac1.abort();
    await c1;
  }

  let count2 = 0;
  const ac2 = new AbortController();
  const c2 = Deno.cron("abc", { minute: { start: 0, every: 20 } }, {
    signal: ac2.signal,
  }, () => {
    count2++;
    if (count2 > 5) {
      resolve();
    }
  });
  try {
    await promise;
  } finally {
    ac2.abort();
    await c2;
  }
});

Deno.test(async function multipleCrons() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count1 = 0;
  let count2 = 0;
  const { promise: promise1, resolve: resolve1 } = Promise.withResolvers<
    void
  >();
  const { promise: promise2, resolve: resolve2 } = Promise.withResolvers<
    void
  >();
  const ac1 = new AbortController();
  const c1 = Deno.cron("abc", "*/20 * * * *", { signal: ac1.signal }, () => {
    count1++;
    if (count1 > 5) {
      resolve1();
    }
  });
  const c2 = Deno.cron("xyz", "*/20 * * * *", { signal: ac1.signal }, () => {
    count2++;
    if (count2 > 5) {
      resolve2();
    }
  });
  try {
    await promise1;
    await promise2;
  } finally {
    ac1.abort();
    await c1;
    await c2;
  }

  let count3 = 0;
  let count4 = 0;
  const { promise: promise3, resolve: resolve3 } = Promise.withResolvers<
    void
  >();
  const { promise: promise4, resolve: resolve4 } = Promise.withResolvers<
    void
  >();
  const ac2 = new AbortController();
  const c3 = Deno.cron("abc", { minute: { start: 0, every: 20 } }, {
    signal: ac2.signal,
  }, () => {
    count3++;
    if (count3 > 5) {
      resolve3();
    }
  });
  const c4 = Deno.cron("xyz", { minute: { start: 0, every: 20 } }, {
    signal: ac2.signal,
  }, () => {
    count4++;
    if (count4 > 5) {
      resolve4();
    }
  });
  try {
    await promise3;
    await promise4;
  } finally {
    ac2.abort();
    await c3;
    await c4;
  }
});

Deno.test(async function overlappingExecutions() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

  let count1 = 0;
  const { promise: promise1, resolve: resolve1 } = Promise.withResolvers<
    void
  >();
  const { promise: promise2, resolve: resolve2 } = Promise.withResolvers<
    void
  >();
  const ac1 = new AbortController();
  const c1 = Deno.cron(
    "abc",
    "*/20 * * * *",
    { signal: ac1.signal },
    async () => {
      resolve1();
      count1++;
      await promise2;
    },
  );
  try {
    await promise1;
  } finally {
    await sleep(2000);
    resolve2();
    ac1.abort();
    await c1;
  }
  assertEquals(count1, 1);

  let count2 = 0;
  const { promise: promise3, resolve: resolve3 } = Promise.withResolvers<
    void
  >();
  const { promise: promise4, resolve: resolve4 } = Promise.withResolvers<
    void
  >();
  const ac2 = new AbortController();
  const c2 = Deno.cron(
    "abc",
    { minute: { start: 0, every: 20 } },
    { signal: ac2.signal },
    async () => {
      resolve3();
      count2++;
      await promise4;
    },
  );
  try {
    await promise3;
  } finally {
    await sleep(2000);
    resolve4();
    ac2.abort();
    await c2;
  }
  assertEquals(count2, 1);
});

Deno.test(async function retriesWithBackoffSchedule() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "5000");

  let count1 = 0;
  const ac1 = new AbortController();
  const c1 = Deno.cron("abc1", "*/20 * * * *", {
    signal: ac1.signal,
    backoffSchedule: [10, 20],
  }, async () => {
    count1 += 1;
    await sleep(10);
    throw new TypeError("cron error");
  });
  try {
    await sleep(6000);
  } finally {
    ac1.abort();
    await c1;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count1, 3);

  let count2 = 0;
  const ac2 = new AbortController();
  const c2 = Deno.cron("abc2", { minute: { start: 0, every: 20 } }, {
    signal: ac2.signal,
    backoffSchedule: [10, 20],
  }, async () => {
    count2 += 1;
    await sleep(10);
    throw new TypeError("cron error");
  });
  try {
    await sleep(6000);
  } finally {
    ac2.abort();
    await c2;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count2, 3);
});

Deno.test(async function retriesWithBackoffScheduleOldApi() {
  Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "5000");

  let count1 = 0;
  const ac1 = new AbortController();
  const c1 = Deno.cron("abc1", "*/20 * * * *", async () => {
    count1 += 1;
    await sleep(10);
    throw new TypeError("cron error");
  }, {
    signal: ac1.signal,
    backoffSchedule: [10, 20],
  });

  try {
    await sleep(6000);
  } finally {
    ac1.abort();
    await c1;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count1, 3);

  let count2 = 0;
  const ac2 = new AbortController();
  const c2 = Deno.cron(
    "abc2",
    { minute: { start: 0, every: 20 } },
    async () => {
      count2 += 1;
      await sleep(10);
      throw new TypeError("cron error");
    },
    {
      signal: ac2.signal,
      backoffSchedule: [10, 20],
    },
  );

  try {
    await sleep(6000);
  } finally {
    ac2.abort();
    await c2;
  }

  // The cron should have executed 3 times (1st attempt and 2 retries).
  assertEquals(count2, 3);
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
