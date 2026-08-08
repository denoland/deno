// Copyright 2018-2026 the Deno authors. MIT license.
import { test } from "checkin:testing";

test(async function testQueueMicrotask() {
  await new Promise((r) =>
    queueMicrotask(() => {
      console.log("In microtask!");
      r(null);
    })
  );
});

// Regression test for denoland/deno#11731: verify that
// Deno.core.invokeUserCallback correctly passes through return values
// and propagates exceptions. The full microtask-checkpoint ordering
// tests live in tests/unit/event_target_test.ts (deno repo) which has
// access to the EventTarget and AbortSignal implementations.
test(function testInvokeUserCallbackReturnAndExceptionSemantics() {
  // deno-lint-ignore no-explicit-any
  const core = (Deno as any).core;
  if (typeof core?.invokeUserCallback !== "function") {
    // invokeUserCallback is not available in this test harness.
    return;
  }
  const { invokeUserCallback } = core;

  // Return value passes through.
  const result = invokeUserCallback(
    (a: number, b: number) => a + b,
    null,
    [3, 4],
  );
  if (result !== 7) {
    throw new Error(`expected 7, got ${result}`);
  }

  // `this` binding works.
  const obj = { x: 10 };
  const got = invokeUserCallback(
    function (this: { x: number }) {
      return this.x;
    },
    obj,
    [],
  );
  if (got !== 10) {
    throw new Error(`expected 10, got ${got}`);
  }

  // Exceptions propagate.
  try {
    invokeUserCallback(
      () => {
        throw new Error("boom");
      },
      null,
      [],
    );
    throw new Error("expected throw");
  } catch (e) {
    if ((e as Error).message !== "boom") {
      throw e;
    }
  }
});
