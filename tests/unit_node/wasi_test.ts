// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import wasi from "node:wasi";
import { assertThrows } from "@std/assert";

Deno.test("[node/wasi] - WASI should throw (not implemented)", () => {
  assertThrows(() => new wasi.WASI());
});
