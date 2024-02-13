// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { earlyZipReadableStreams } from "./early_zip_readable_streams.ts";
import { assertEquals } from "../assert/mod.ts";

Deno.test("[streams] earlyZipReadableStreams short first", async () => {
  const textStream = ReadableStream.from(["1", "2", "3"]);
  const textStream2 = ReadableStream.from(["a", "b", "c", "d", "e"]);

  const buf = await Array.fromAsync(
    earlyZipReadableStreams(textStream, textStream2),
  );

  assertEquals(buf, [
    "1",
    "a",
    "2",
    "b",
    "3",
    "c",
  ]);
});

Deno.test("[streams] earlyZipReadableStreams long first", async () => {
  const textStream = ReadableStream.from(["a", "b", "c", "d", "e"]);
  const textStream2 = ReadableStream.from(["1", "2", "3"]);

  const buf = await Array.fromAsync(
    earlyZipReadableStreams(textStream, textStream2),
  );

  assertEquals(buf, [
    "a",
    "1",
    "b",
    "2",
    "c",
    "3",
    "d",
  ]);
});
