// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/assert_equals.ts";
import { toArrayBuffer } from "./to_array_buffer.ts";

Deno.test("[streams] toArrayBuffer", async () => {
  const stream = ReadableStream.from([
    new Uint8Array([1, 2, 3, 4, 5]),
    new Uint8Array([6, 7]),
    new Uint8Array([8, 9]),
  ]);

  const buf = await toArrayBuffer(stream);
  assertEquals(buf, new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9]).buffer);
});
