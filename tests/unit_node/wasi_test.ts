// Copyright 2018-2025 the Deno authors. MIT license.
import wasi from "node:wasi";
import { assertThrows } from "@std/assert";

Deno.test("[node/wasi] - WASI should throw (not implemented)", () => {
  assertThrows(() => new wasi.WASI());
});
