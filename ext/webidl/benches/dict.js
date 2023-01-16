// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const { createDictionaryConverter, converters } = globalThis.__bootstrap.webidl;

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
