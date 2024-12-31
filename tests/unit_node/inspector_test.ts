// Copyright 2018-2025 the Deno authors. MIT license.
import inspector, { Session } from "node:inspector";
import inspectorPromises, {
  Session as SessionPromise,
} from "node:inspector/promises";
import { assertEquals } from "@std/assert/equals";

Deno.test("[node/inspector] - importing inspector works", () => {
  assertEquals(typeof inspector.open, "function");
});

Deno.test("[node/inspector] - Session constructor should not throw", () => {
  new Session();
});

Deno.test("[node/inspector/promises] - importing inspector works", () => {
  assertEquals(typeof inspectorPromises.open, "function");
});

Deno.test("[node/inspector/promises] - Session constructor should not throw", () => {
  new SessionPromise();
});
