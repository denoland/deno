// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { ConcatenatedJsonParseStream } from "./concatenated_json_parse_stream.ts";
import { assertInvalidParse, assertValidParse } from "./_test_common.ts";

Deno.test({
  name: "[json] ConcatenatedJsonParseStream",
  async fn() {
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"}'],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"} '],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      [' {"foo": "bar"}'],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['[{"foo": "bar"}]'],
      [[{ foo: "bar" }]],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"}{"foo": "bar"}'],
      [{ foo: "bar" }, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"} {"foo": "bar"}'],
      [{ foo: "bar" }, { foo: "bar" }],
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: primitive",
  async fn() {
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["0"],
      [0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["100"],
      [100],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['100 200"foo"'],
      [100, 200, "foo"],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['100 200{"foo": "bar"}'],
      [100, 200, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['100 200["foo"]'],
      [100, 200, ["foo"]],
    );

    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['"foo"'],
      ["foo"],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['"foo""bar"{"foo": "bar"}'],
      ["foo", "bar", { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['"foo""bar"["foo"]'],
      ["foo", "bar", ["foo"]],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['"foo""bar"0'],
      ["foo", "bar", 0],
    );

    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["null"],
      [null],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['null null{"foo": "bar"}'],
      [null, null, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['null null["foo"]'],
      [null, null, ["foo"]],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["null null 0"],
      [null, null, 0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['null null"foo"'],
      [null, null, "foo"],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["nullnull"],
      [null, null],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["nullnull0"],
      [null, null, 0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['nullnull"foo"'],
      [null, null, "foo"],
    );

    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["true"],
      [true],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['true true{"foo": "bar"}'],
      [true, true, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['true true["foo"]'],
      [true, true, ["foo"]],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["true true 0"],
      [true, true, 0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['true true"foo"'],
      [true, true, "foo"],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["truetrue"],
      [true, true],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["truetrue0"],
      [true, true, 0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['truetrue"foo"'],
      [true, true, "foo"],
    );

    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["false"],
      [false],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['false false{"foo": "bar"}'],
      [false, false, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['false false["foo"]'],
      [false, false, ["foo"]],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["false false 0"],
      [false, false, 0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['false false"foo"'],
      [false, false, "foo"],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["falsefalse"],
      [false, false],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["falsefalse0"],
      [false, false, 0],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['falsefalse"foo"'],
      [false, false, "foo"],
    );

    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['nullfalsetrue0true"foo"falsenullnull'],
      [null, false, true, 0, true, "foo", false, null, null],
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: chunk",
  async fn() {
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["", '{"foo": "bar"}'],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["{", '"foo": "bar"}'],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "b', 'ar"}'],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"', "}"],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"}', ""],
      [{ foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"}', '{"foo": "bar"}'],
      [{ foo: "bar" }, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"', '}{"foo": "bar"}'],
      [{ foo: "bar" }, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"}{', '"foo": "bar"}'],
      [{ foo: "bar" }, { foo: "bar" }],
    );
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["tr", 'ue{"foo": "bar"}'],
      [true, { foo: "bar" }],
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: surrogate pair",
  async fn() {
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "👪"}{"foo": "👪"}'],
      [{ foo: "👪" }, { foo: "👪" }],
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: symbol between double quotes",
  async fn() {
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ['"[], {}"'],
      ["[], {}"],
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: primitives in containers",
  async fn() {
    await assertValidParse(
      ConcatenatedJsonParseStream,
      ["[ true ]"],
      [[true]],
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: halfway chunk",
  async fn() {
    await assertInvalidParse(
      ConcatenatedJsonParseStream,
      ['{"foo": "bar"} {"foo": '],
      {},
      SyntaxError,
      `Unexpected end of JSON input (parsing: ' {"foo": ')`,
    );
  },
});

Deno.test({
  name: "[json] ConcatenatedJsonParseStream: truncate error message",
  async fn() {
    await assertInvalidParse(
      ConcatenatedJsonParseStream,
      [`{${"foo".repeat(100)}}`],
      {},
      SyntaxError,
      `Expected property name or '}' in JSON at position 1 (line 1 column 2) (parsing: '{foofoofoofoofoofoofoofoofoofo...')`,
    );
  },
});

Deno.test({
  // Read the test data file
  name: "[json] parse: testdata(concatenated-json)",
  async fn() {
    const url = "./testdata/test.concatenated-json";
    const { body } = await fetch(new URL(url, import.meta.url).toString());
    const readable = body!
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new ConcatenatedJsonParseStream());

    const result = await Array.fromAsync(readable);

    assertEquals(result, [
      { "hello": "world" },
      ["👋", "👋", "👋"],
      { "deno": "🦕" },
    ]);
  },
});
