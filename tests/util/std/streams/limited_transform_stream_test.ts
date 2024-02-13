// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertRejects } from "../assert/mod.ts";
import { LimitedTransformStream } from "./limited_transform_stream.ts";

Deno.test("[streams] LimitedTransformStream", async function () {
  const r = ReadableStream.from([
    "foo",
    "foo",
    "foo",
    "foo",
    "foo",
    "foo",
  ]).pipeThrough(new LimitedTransformStream(3));

  const chunks = await Array.fromAsync(r);
  assertEquals(chunks.length, 3);
});

Deno.test("[streams] LimitedTransformStream error", async function () {
  const r = ReadableStream.from([
    "foo",
    "foo",
    "foo",
    "foo",
    "foo",
    "foo",
  ]).pipeThrough(new LimitedTransformStream(3, { error: true }));

  await assertRejects(async () => await Array.fromAsync(r), RangeError);
});
