// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { concat } from "../bytes/concat.ts";

export async function toArrayBuffer(
  readableStream: ReadableStream<Uint8Array>,
): Promise<ArrayBuffer> {
  const reader = readableStream.getReader();
  const chunks: Uint8Array[] = [];

  while (true) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    chunks.push(value);
  }

  return concat(chunks).buffer;
}
