// Copyright 2018-2026 the Deno authors. MIT license.

import { context, createContextKey } from "npm:@opentelemetry/api@1.9.0";

function assertEquals(actual: unknown, expected: unknown, msg?: string) {
  if (actual !== expected) {
    throw new Error(
      `assertEquals failed${msg ? ` (${msg})` : ""}: ${String(actual)} !== ${
        String(expected)
      }`,
    );
  }
}

function assertNotStrictEquals(a: unknown, b: unknown, msg?: string) {
  if (a === b) {
    throw new Error(`assertNotStrictEquals failed${msg ? ` (${msg})` : ""}`);
  }
}

const { contextManager: ContextManager } = Deno.telemetry;

// Plain (string) keys: set/get/delete with immutable copy semantics.
const a = ContextManager.active();
const b = a.setValue("b", 1);
const c = b.setValue("c", 2);

const subB = c.deleteValue("b");
const subC = subB.deleteValue("c");

assertEquals(a.getValue("b"), undefined);
assertEquals(b.getValue("b"), 1);
assertEquals(c.getValue("b"), 1);

assertEquals(a.getValue("c"), undefined);
assertEquals(b.getValue("c"), undefined);
assertEquals(c.getValue("c"), 2);

assertEquals(subB.getValue("b"), undefined);
assertEquals(subB.getValue("c"), 2);

assertEquals(subC.getValue("b"), undefined);
assertEquals(subC.getValue("c"), undefined);

// set/delete must return a brand new context, never mutate the receiver.
assertNotStrictEquals(a, b);
assertNotStrictEquals(b, c);
assertNotStrictEquals(c, subB);

// Re-setting an existing key updates the value but leaves the original intact.
const c2 = c.setValue("c", 3);
assertEquals(c.getValue("c"), 2);
assertEquals(c2.getValue("c"), 3);

// Symbol keys (how `@opentelemetry/api` actually stores values): identity is
// compared via the global symbol registry, so `createContextKey` round-trips.
const KEY = createContextKey("My Context Key");
const KEY2 = createContextKey("Other Context Key");
const withKey = ContextManager.active().setValue(KEY, "value");
assertEquals(withKey.getValue(KEY), "value");
assertEquals(withKey.getValue(KEY2), undefined);
assertEquals(ContextManager.active().getValue(KEY), undefined);

// `@opentelemetry/api`'s context API delegates to the registered manager, and
// the async-context value is restored once `with` returns.
const apiKey = createContextKey("API Key");
const result = context.with(
  context.active().setValue(apiKey, 42),
  () => context.active().getValue(apiKey),
);
assertEquals(result, 42);
assertEquals(context.active().getValue(apiKey), undefined);

console.log("context ok");
