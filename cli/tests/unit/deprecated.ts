// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, unitTest } from "./test_util.ts";

const {
  deprecatedMap,
  deprecated,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

unitTest(function deprecatedMapShouldBeMap() {
  assert(deprecatedMap instanceof Map);
});

unitTest(function deprecatedShouldBeFunction() {
  assert(typeof deprecated === "function");
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mockWarn(): { calls: any[][]; remove(): void } {
  const origConsoleWarn = Object.getOwnPropertyDescriptor(
    globalThis.console,
    "warn",
  );
  assert(origConsoleWarn);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const calls: any[][] = [];
  Object.defineProperty(globalThis.console, "warn", {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value(...args: any[]) {
      calls.push(args);
    },
    configurable: true,
  });
  return {
    calls,
    remove() {
      Object.defineProperty(globalThis.console, "warn", origConsoleWarn);
    },
  };
}

unitTest(function deprecatedShouldWarn() {
  const m = mockWarn();
  assert(!deprecatedMap.has("mock-feature"));
  deprecated("mock-feature", "Mock feature has been deprecated");
  assertEquals(m.calls, [["Mock feature has been deprecated"]]);
  assert(deprecatedMap.has("mock-feature"));
  m.remove();
});

unitTest(function deprecatedShouldWarnOnce() {
  const m = mockWarn();
  assert(!deprecatedMap.has("mock-feature-a"));
  deprecated("mock-feature-a", "Has been deprecated");
  deprecated("mock-feature-a", "Should not be sent");
  assertEquals(m.calls, [["Has been deprecated"]]);
  m.remove();
});

unitTest(function deprecatedShouldAllowForce() {
  const m = mockWarn();
  assert(!deprecatedMap.has("mock-feature-b"));
  deprecated("mock-feature-b", "Has been deprecated");
  deprecated("mock-feature-b", "Should not be sent");
  deprecated("mock-feature-b", "Can be forced", true);
  assertEquals(m.calls, [["Has been deprecated"], ["Can be forced"]]);
  m.remove();
});

unitTest(function deprecatedShouldHaveDefaultMsg() {
  const m = mockWarn();
  assert(!deprecatedMap.has("mock-feature-c"));
  deprecated("mock-feature-c");
  assertEquals(m.calls, [[`The feature "mock-feature-c" is deprecated.`]]);
  m.remove();
});

unitTest(function deprecatedCanBePrepopulated() {
  const m = mockWarn();
  assert(!deprecatedMap.has("mock-feature-d"));
  deprecatedMap.set("mock-feature-d", {
    msg: "This value is already set",
    flag: false,
  });
  deprecated("mock-feature-d");
  deprecated("mock-feature-d");
  assertEquals(m.calls, [["This value is already set"]]);
  m.remove();
});
