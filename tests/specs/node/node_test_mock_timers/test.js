// Copyright 2018-2026 the Deno authors. MIT license.
import assert from "node:assert/strict";
import test, { mock } from "node:test";

test("reproducer from issue 32987 does not throw", () => {
  mock.timers.enable({ apis: ["setInterval", "Date"], now: 1234 });
  assert.strictEqual(Date.now(), 1234);
  mock.timers.reset();
});

test("setTimeout fires on tick", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  let called = false;
  setTimeout(() => {
    called = true;
  }, 100);
  assert.strictEqual(called, false);
  mock.timers.tick(50);
  assert.strictEqual(called, false);
  mock.timers.tick(50);
  assert.strictEqual(called, true);
  mock.timers.reset();
});

test("setInterval fires repeatedly on tick", () => {
  mock.timers.enable({ apis: ["setInterval"] });
  let count = 0;
  const id = setInterval(() => {
    count++;
  }, 100);
  mock.timers.tick(550);
  assert.strictEqual(count, 5);
  clearInterval(id);
  mock.timers.tick(500);
  assert.strictEqual(count, 5);
  mock.timers.reset();
});

test("clearTimeout cancels a mocked timer", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  let fired = false;
  const id = setTimeout(() => {
    fired = true;
  }, 100);
  clearTimeout(id);
  mock.timers.tick(200);
  assert.strictEqual(fired, false);
  mock.timers.reset();
});

test("clearTimeout works when only setTimeout is requested", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  let fired = false;
  const id = setTimeout(() => {
    fired = true;
  }, 100);
  clearTimeout(id);
  mock.timers.tick(200);
  assert.strictEqual(fired, false);
  mock.timers.reset();
});

test("timer handle coerces to numeric id", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  let fired = false;
  const id = setTimeout(() => {
    fired = true;
  }, 100);
  assert.strictEqual(typeof +id, "number");
  clearTimeout(+id);
  mock.timers.tick(100);
  assert.strictEqual(fired, false);
  mock.timers.reset();
});

test("timeout handle refresh restarts fired timeout", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  let count = 0;
  const id = setTimeout(() => {
    count++;
  }, 100);
  mock.timers.tick(100);
  assert.strictEqual(count, 1);
  id.refresh();
  mock.timers.tick(99);
  assert.strictEqual(count, 1);
  mock.timers.tick(1);
  assert.strictEqual(count, 2);
  mock.timers.reset();
});

test("setImmediate fires on tick", () => {
  mock.timers.enable({ apis: ["setImmediate"] });
  let fired = false;
  setImmediate(() => {
    fired = true;
  });
  mock.timers.tick(0);
  assert.strictEqual(fired, true);
  mock.timers.reset();
});

test("setImmediate runs before zero-delay setTimeout", () => {
  mock.timers.enable({ apis: ["setImmediate", "setTimeout"] });
  const calls = [];
  setTimeout(() => {
    calls.push("timeout");
  }, 0);
  setImmediate(() => {
    calls.push("immediate");
  });
  mock.timers.tick(0);
  assert.deepStrictEqual(calls, ["immediate", "timeout"]);
  mock.timers.reset();
});

test("timeout overflow is coerced to one millisecond", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  let fired = false;
  setTimeout(() => {
    fired = true;
  }, 2147483648);
  mock.timers.tick(1);
  assert.strictEqual(fired, true);
  mock.timers.reset();
});

test("runAll runs all pending timers", () => {
  mock.timers.enable();
  let a = 0, b = 0, c = 0;
  setTimeout(() => {
    a++;
  }, 100);
  setTimeout(() => {
    b++;
  }, 50);
  setTimeout(() => {
    c++;
  }, 1000);
  mock.timers.runAll();
  assert.strictEqual(a, 1);
  assert.strictEqual(b, 1);
  assert.strictEqual(c, 1);
  mock.timers.reset();
});

test("runAll fires each interval exactly once", () => {
  mock.timers.enable({ apis: ["setInterval"] });
  let count = 0;
  setInterval(() => {
    count++;
  }, 100);
  mock.timers.runAll();
  assert.strictEqual(count, 1);
  mock.timers.reset();
});

test("Date is mocked and tracks tick", () => {
  mock.timers.enable({ apis: ["Date"], now: 5000 });
  assert.strictEqual(Date.now(), 5000);
  assert.strictEqual(new Date().getTime(), 5000);
  assert.strictEqual(Date.isMock, true);
  assert.strictEqual(Date.toString(), "function Date() { [native code] }");
  assert.match(Date(), /1970/);
  // explicit timestamp still works
  assert.strictEqual(new Date(0).getTime(), 0);
  mock.timers.reset();
});

test("Date with `now` as Date instance", () => {
  mock.timers.enable({ apis: ["Date"], now: new Date(2000) });
  assert.strictEqual(Date.now(), 2000);
  mock.timers.reset();
});

test("Date.parse and Date.UTC still work when mocked", () => {
  mock.timers.enable({ apis: ["Date"], now: 0 });
  assert.strictEqual(typeof Date.parse("2020-01-01"), "number");
  assert.strictEqual(typeof Date.UTC(2020, 0, 1), "number");
  mock.timers.reset();
});

test("tick advances Date when both are mocked", () => {
  mock.timers.enable({ apis: ["setTimeout", "Date"], now: 0 });
  setTimeout(() => {}, 100);
  mock.timers.tick(500);
  assert.strictEqual(Date.now(), 500);
  mock.timers.reset();
});

test("setTime sets clock without firing timers", () => {
  mock.timers.enable();
  let fired = false;
  setTimeout(() => {
    fired = true;
  }, 100);
  mock.timers.setTime(50000);
  assert.strictEqual(Date.now(), 50000);
  assert.strictEqual(fired, false);
  mock.timers.reset();
});

test("reset restores original globals", () => {
  const realSetTimeout = globalThis.setTimeout;
  const realDate = globalThis.Date;
  mock.timers.enable();
  assert.notStrictEqual(globalThis.setTimeout, realSetTimeout);
  assert.notStrictEqual(globalThis.Date, realDate);
  mock.timers.reset();
  assert.strictEqual(globalThis.setTimeout, realSetTimeout);
  assert.strictEqual(globalThis.Date, realDate);
});

test("Symbol.dispose resets original globals", () => {
  const realSetTimeout = globalThis.setTimeout;
  {
    using timers = mock.timers;
    timers.enable({ apis: ["setTimeout"] });
    assert.notStrictEqual(globalThis.setTimeout, realSetTimeout);
  }
  assert.strictEqual(globalThis.setTimeout, realSetTimeout);
});

test("throws ERR_INVALID_STATE when not enabled", () => {
  assert.throws(() => mock.timers.tick(100), { code: "ERR_INVALID_STATE" });
  assert.throws(() => mock.timers.runAll(), { code: "ERR_INVALID_STATE" });
  assert.throws(() => mock.timers.setTime(0), { code: "ERR_INVALID_STATE" });
});

test("throws ERR_INVALID_STATE when enabling twice", () => {
  mock.timers.enable();
  assert.throws(() => mock.timers.enable(), { code: "ERR_INVALID_STATE" });
  mock.timers.reset();
});

test("throws on invalid api name", () => {
  assert.throws(() => mock.timers.enable({ apis: ["unknown"] }), {
    code: "ERR_INVALID_ARG_VALUE",
  });
  assert.throws(() => mock.timers.enable({ apis: ["clearTimeout"] }), {
    code: "ERR_INVALID_ARG_VALUE",
  });
});
