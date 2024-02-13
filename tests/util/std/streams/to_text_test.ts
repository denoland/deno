// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/assert_equals.ts";
import { toText } from "./to_text.ts";

Deno.test("[streams] toText", async () => {
  const byteStream = ReadableStream.from(["hello", " js ", "fans"])
    .pipeThrough(new TextEncoderStream());

  assertEquals(await toText(byteStream), "hello js fans");

  const stringStream = ReadableStream.from(["hello", " deno ", "world"]);

  assertEquals(await toText(stringStream), "hello deno world");
});
