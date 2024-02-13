// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/assert_equals.ts";
import { toJson } from "./to_json.ts";

Deno.test("[streams] toJson", async () => {
  const byteStream = ReadableStream.from(["[", "1, 2, 3, 4", "]"])
    .pipeThrough(new TextEncoderStream());

  assertEquals(await toJson(byteStream), [1, 2, 3, 4]);

  const stringStream = ReadableStream.from([
    '{ "a": 2,',
    ' "b": 3,',
    ' "c": 4 }',
  ]);

  assertEquals(await toJson(stringStream), {
    a: 2,
    b: 3,
    c: 4,
  });
});
