// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

const textDecoder = new TextDecoder();

export async function toText(
  readableStream: ReadableStream,
): Promise<string> {
  const reader = readableStream.getReader();
  let result = "";

  while (true) {
    const { done, value } = await reader.read();

    if (done) {
      break;
    }

    result += typeof value === "string" ? value : textDecoder.decode(value);
  }

  return result;
}
