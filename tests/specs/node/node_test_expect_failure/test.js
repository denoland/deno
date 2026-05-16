// deno-lint-ignore-file

import assert from "node:assert";
import test from "node:test";

test("test.expectFailure is a function", () => {
  assert.strictEqual(typeof test.expectFailure, "function");
});

test.expectFailure("sync expected failure", () => {
  assert.fail("expected sync failure");
});

test("async expected failure option", { expectFailure: true }, async () => {
  throw new Error("expected async failure");
});

test("matched expected failure", { expectFailure: /expected match/ }, () => {
  throw new Error("expected match");
});

test.expectFailure("callback expected failure", (t, done) => {
  done(new Error("expected callback failure"));
});

test.expectFailure("unexpected pass fails the test", () => {});

test("mismatched expected failure fails the test", {
  expectFailure: /expected message/,
}, () => {
  throw new Error("actual message");
});
