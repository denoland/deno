// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { TextDelimiterStream } from "../streams/text_delimiter_stream.ts";
import { TextLineStream } from "../streams/text_line_stream.ts";
import { JsonParseStream } from "./json_parse_stream.ts";
import { assertInvalidParse, assertValidParse } from "./_test_common.ts";

Deno.test({
  name: "[json] JsonParseStream",
  async fn() {
    await assertValidParse(
      JsonParseStream,
      ['{"foo": "bar"}', '["foo"]', '"foo"', "0", "null", "true", "false"],
      [{ foo: "bar" }, ["foo"], "foo", 0, null, true, false],
    );
  },
});

Deno.test({
  name: "[json] JsonParseStream: empty line",
  async fn() {
    await assertValidParse(
      JsonParseStream,
      [" \t\r\n", ""],
      [],
    );
  },
});

Deno.test({
  name: "[json] JsonParseStream: special character",
  async fn() {
    await assertValidParse(
      JsonParseStream,
      ['"👪"', '"🦕"', '"\u3053\u3093\u306b\u3061\u306f"'],
      ["👪", "🦕", "\u3053\u3093\u306b\u3061\u306f"],
    );
  },
});

Deno.test({
  name: "[json] JsonParseStream: expect error",
  async fn() {
    await assertInvalidParse(
      JsonParseStream,
      ['{"foo": "bar"}', '{"foo": '],
      {},
      SyntaxError,
      `Unexpected end of JSON input (parsing: '{"foo": ')`,
    );
  },
});

Deno.test({
  name: "[json] parse: testdata(jsonl)",
  async fn() {
    // Read the test data file
    const url = "./testdata/test.jsonl";
    const { body } = await fetch(new URL(url, import.meta.url).toString());
    const readable = body!
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new TextLineStream())
      .pipeThrough(new JsonParseStream());

    const result = await Array.fromAsync(readable);

    assertEquals(result, [
      { "hello": "world" },
      ["👋", "👋", "👋"],
      { "deno": "🦕" },
    ]);
  },
});

Deno.test({
  name: "[json] parse: testdata(ndjson)",
  async fn() {
    // Read the test data file
    const url = "./testdata/test.ndjson";
    const { body } = await fetch(new URL(url, import.meta.url).toString());
    const readable = body!
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new TextLineStream())
      .pipeThrough(new JsonParseStream());

    const result = await Array.fromAsync(readable);

    assertEquals(result, [
      { "hello": "world" },
      ["👋", "👋", "👋"],
      { "deno": "🦕" },
    ]);
  },
});

Deno.test({
  name: "[json] parse: testdata(json-seq)",
  async fn() {
    // Read the test data file
    const recordSeparator = "\x1E";

    const url = "./testdata/test.json-seq";
    const { body } = await fetch(new URL(url, import.meta.url).toString());
    const readable = body!
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new TextDelimiterStream(recordSeparator))
      .pipeThrough(new JsonParseStream());

    const result = await Array.fromAsync(readable);

    assertEquals(result, [
      { "hello": "world" },
      ["👋", "👋", "👋"],
      { "deno": "🦕" },
    ]);
  },
});
