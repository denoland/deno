// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
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

Deno.test("[node/inspector/promise] - importing inspector works", () => {
  assertEquals(typeof inspectorPromises.open, "function");
});

Deno.test("[node/inspector/promise] - Session constructor should not throw", () => {
  new SessionPromise();
});
