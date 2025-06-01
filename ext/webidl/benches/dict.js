// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file

import {
  converters,
  createDictionaryConverter,
} from "ext:deno_webidl/00_webidl.js";

const TextDecodeOptions = createDictionaryConverter(
  "TextDecodeOptions",
  [
    {
      key: "stream",
      converter: converters.boolean,
      defaultValue: false,
    },
  ],
);
globalThis.TextDecodeOptions = TextDecodeOptions;

// Sanity check
{
  const o = TextDecodeOptions(undefined);
  if (o.stream !== false) {
    throw new Error("Unexpected stream value");
  }
}

function handwrittenConverter(V) {
  const defaultValue = { stream: false };
  if (V === undefined || V === null) {
    return defaultValue;
  }
  if (V.stream !== undefined) {
    defaultValue.stream = !!V.stream;
  }
  return defaultValue;
}
globalThis.handwrittenConverter = handwrittenConverter;
