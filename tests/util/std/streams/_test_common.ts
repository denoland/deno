// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../assert/mod.ts";

// N controls how many iterations of certain checks are performed.
const N = 100;

export function init(): Uint8Array {
  const testBytes = new Uint8Array(N);
  for (let i = 0; i < N; i++) {
    testBytes[i] = "a".charCodeAt(0) + (i % 26);
  }
  return testBytes;
}

/**
 * Verify that a transform stream produces the expected output data
 * @param transform The transform stream to test
 * @param inputs Source input data
 * @param outputs Expected output data
 */
export async function testTransformStream<T, U>(
  transform: TransformStream<T, U>,
  inputs: Iterable<T> | AsyncIterable<T>,
  outputs: Iterable<U> | AsyncIterable<U>,
) {
  const reader = ReadableStream.from(inputs)
    .pipeThrough(transform)
    .getReader();
  for await (const output of outputs) {
    const { value, done } = await reader.read();
    assertEquals(value, output);
    assertEquals(done, false);
  }
  const f = await reader.read();
  assert(f.done, `stream not done, value was: ${f.value}`);
}
