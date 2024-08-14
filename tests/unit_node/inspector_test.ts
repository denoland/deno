// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import inspector from "node:inspector";
import { assertEquals } from "@std/assert/equals";

Deno.test("[node/inspector] - importing inspector works", () => {
  assertEquals(typeof inspector.open, "function");
});
