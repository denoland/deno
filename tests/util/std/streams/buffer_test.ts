// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../assert/mod.ts";
import { Buffer } from "./buffer.ts";

Deno.test("[streams] Buffer Write & Read", async function () {
  const buf = new Buffer();
  const writer = buf.writable.getWriter();
  const reader = buf.readable.getReader({ mode: "byob" });
  const data = new Uint8Array([4, 21, 45, 19]);
  await writer.write(data);
  const read = await reader.read(new Uint8Array(4));
  assertEquals(read.value, data);
});

Deno.test("[streams] Buffer Read empty", async function () {
  const buf = new Buffer();
  const reader = buf.readable.getReader({ mode: "byob" });
  const read = await reader.read(new Uint8Array(5));
  assert(read.done);
  assertEquals(read.value!.byteLength, 0);
});

Deno.test("[streams] Buffer Write & get bytes", async function () {
  const buf = new Buffer();
  const writer = buf.writable.getWriter();
  const data = new Uint8Array([4, 21, 45, 19]);
  await writer.write(data);
  assertEquals(buf.bytes(), data);
});

Deno.test("[streams] Buffer truncate", async function () {
  const buf = new Buffer();
  const writer = buf.writable.getWriter();
  await writer.write(new Uint8Array([4, 21, 45, 19]));
  buf.truncate(3);
  assertEquals(buf.bytes(), new Uint8Array([4, 21, 45]));
});
