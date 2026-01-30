// Copyright 2018-2026 the Deno authors. MIT license.
import assert from "node:assert/strict";
import test, { mock } from "node:test";

test("mock.fn() creates a mock function", () => {
  const fn = mock.fn();
  assert.strictEqual(typeof fn, "function");
  assert.strictEqual(fn.mock.callCount(), 0);

  fn();
  assert.strictEqual(fn.mock.callCount(), 1);

  fn("arg1", "arg2");
  assert.strictEqual(fn.mock.callCount(), 2);
  assert.deepStrictEqual(fn.mock.calls[0].arguments, []);
  assert.deepStrictEqual(fn.mock.calls[1].arguments, ["arg1", "arg2"]);

  mock.restoreAll();
});

test("mock.fn() with implementation", () => {
  const fn = mock.fn((a, b) => a + b);
  const result = fn(1, 2);
  assert.strictEqual(result, 3);
  assert.strictEqual(fn.mock.callCount(), 1);
  assert.strictEqual(fn.mock.calls[0].result, 3);

  mock.restoreAll();
});

test("mock.fn() with original and implementation override", () => {
  const original = (x) => x * 2;
  const impl = (x) => x * 3;
  const fn = mock.fn(original, impl);

  assert.strictEqual(fn(5), 15);
  assert.strictEqual(fn.mock.callCount(), 1);

  mock.restoreAll();
});

test("mock.fn() tracks errors", () => {
  const fn = mock.fn(() => {
    throw new Error("test error");
  });
  assert.throws(() => fn(), { message: "test error" });
  assert.strictEqual(fn.mock.callCount(), 1);
  assert.strictEqual(fn.mock.calls[0].error.message, "test error");
  assert.strictEqual(fn.mock.calls[0].result, undefined);

  mock.restoreAll();
});

test("mock.fn() tracks this context", () => {
  const fn = mock.fn(function () {
    return this;
  });
  const obj = { fn };
  obj.fn();
  assert.strictEqual(fn.mock.calls[0].this, obj);

  mock.restoreAll();
});

test("mock.fn() with times option", () => {
  const original = () => "original";
  const impl = () => "mocked";
  const fn = mock.fn(original, impl, { times: 2 });

  assert.strictEqual(fn(), "mocked");
  assert.strictEqual(fn(), "mocked");
  assert.strictEqual(fn(), "original");
  assert.strictEqual(fn.mock.callCount(), 3);

  mock.restoreAll();
});

test("mock.method() mocks an object method", () => {
  const obj = {
    add(a, b) {
      return a + b;
    },
  };

  const mockMethod = mock.method(obj, "add", () => 42);
  assert.strictEqual(obj.add(1, 2), 42);
  assert.strictEqual(mockMethod.mock.callCount(), 1);
  assert.deepStrictEqual(mockMethod.mock.calls[0].arguments, [1, 2]);

  // Restore and verify original is back
  mockMethod.mock.restore();
  assert.strictEqual(obj.add(1, 2), 3);

  mock.restoreAll();
});

test("mock.method() as a spy", () => {
  const obj = {
    greet(name) {
      return "hello " + name;
    },
  };

  mock.method(obj, "greet");
  assert.strictEqual(obj.greet("world"), "hello world");
  assert.strictEqual(obj.greet.mock.callCount(), 1);
  assert.deepStrictEqual(obj.greet.mock.calls[0].arguments, ["world"]);
  assert.strictEqual(obj.greet.mock.calls[0].result, "hello world");

  mock.restoreAll();
});

test("mock.method() throws for non-function property", () => {
  const obj = { value: 42 };
  assert.throws(
    () => mock.method(obj, "value"),
    { message: "Cannot mock property 'value' because it is not a function" },
  );

  mock.restoreAll();
});

test("mock.reset() clears call history of all mocks", () => {
  const fn1 = mock.fn();
  const fn2 = mock.fn();

  fn1();
  fn1();
  fn2();

  assert.strictEqual(fn1.mock.callCount(), 2);
  assert.strictEqual(fn2.mock.callCount(), 1);

  mock.reset();

  assert.strictEqual(fn1.mock.callCount(), 0);
  assert.strictEqual(fn2.mock.callCount(), 0);

  mock.restoreAll();
});

test("mock.restoreAll() restores all mocked methods", () => {
  const obj = {
    greet() {
      return "hello";
    },
    farewell() {
      return "goodbye";
    },
  };

  mock.method(obj, "greet", () => "mocked hello");
  mock.method(obj, "farewell", () => "mocked goodbye");

  assert.strictEqual(obj.greet(), "mocked hello");
  assert.strictEqual(obj.farewell(), "mocked goodbye");

  mock.restoreAll();

  assert.strictEqual(obj.greet(), "hello");
  assert.strictEqual(obj.farewell(), "goodbye");
});

test("test.mock is the same as mock", () => {
  assert.strictEqual(test.mock, mock);
  assert.strictEqual(typeof test.mock.fn, "function");
  assert.strictEqual(typeof test.mock.method, "function");
  assert.strictEqual(typeof test.mock.reset, "function");
  assert.strictEqual(typeof test.mock.restoreAll, "function");
});

test("MockFunctionContext.resetCalls()", () => {
  const fn = mock.fn();
  fn(1);
  fn(2);
  assert.strictEqual(fn.mock.callCount(), 2);

  fn.mock.resetCalls();
  assert.strictEqual(fn.mock.callCount(), 0);
  assert.strictEqual(fn.mock.calls.length, 0);

  mock.restoreAll();
});

test("mock calls have stack traces", () => {
  const fn = mock.fn();
  fn();
  assert.ok(fn.mock.calls[0].stack instanceof Error);

  mock.restoreAll();
});

test("done callback support", (t, done) => {
  setTimeout(() => done(), 10);
});

test("done callback with error", (t, done) => {
  setTimeout(() => done(new Error("callback error")), 10);
});
