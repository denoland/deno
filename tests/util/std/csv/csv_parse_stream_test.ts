// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CsvParseStream } from "./csv_parse_stream.ts";
import type { CsvParseStreamOptions } from "./csv_parse_stream.ts";
import { ERR_QUOTE, ParseError } from "./_io.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
} from "../assert/mod.ts";
import type { AssertTrue, IsExact } from "../testing/types.ts";
import { fromFileUrl, join } from "../path/mod.ts";
import { delay } from "../async/delay.ts";

const testdataDir = join(fromFileUrl(import.meta.url), "../testdata");
const encoder = new TextEncoder();

Deno.test({
  name: "[csv/csv_parse_stream] CsvParseStream should work with Deno.File",
  permissions: {
    read: [testdataDir],
  },
  fn: async () => {
    const file = await Deno.open(join(testdataDir, "simple.csv"));
    const readable = file.readable
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new CsvParseStream());
    const records = await Array.fromAsync(readable);
    assertEquals(records, [
      ["id", "name"],
      ["1", "foobar"],
      ["2", "barbaz"],
    ]);
  },
});

Deno.test({
  name: "[csv/csv_parse_stream] CsvParseStream with invalid csv",
  fn: async () => {
    const readable = ReadableStream.from([
      encoder.encode("id,name\n"),
      encoder.encode("\n"),
      encoder.encode("1,foo\n"),
      encoder.encode('2,"baz\n'),
    ]).pipeThrough(new TextDecoderStream()).pipeThrough(
      new CsvParseStream(),
    );
    const reader = readable.getReader();
    assertEquals(await reader.read(), { done: false, value: ["id", "name"] });
    assertEquals(await reader.read(), { done: false, value: ["1", "foo"] });
    const error = await assertRejects(() => reader.read());
    assert(error instanceof ParseError);
    assertEquals(error.startLine, 4);
    assertEquals(error.line, 5);
    assertEquals(error.column, 0);
    assertStringIncludes(error.message, ERR_QUOTE);
  },
});

Deno.test({
  name: "[csv/csv_parse_stream] CsvParseStream with various inputs",
  permissions: "none",
  fn: async (t) => {
    // These test cases were originally ported from Go:
    // https://github.com/golang/go/blob/go1.12.5/src/encoding/csv/
    // Copyright 2011 The Go Authors. All rights reserved. BSD license.
    // https://github.com/golang/go/blob/master/LICENSE
    const testCases = [
      {
        name: "CRLF",
        input: "a,b\r\nc,d\r\n",
        output: [["a", "b"], ["c", "d"]],
      },
      {
        name: "BareCR",
        input: "a,b\rc,d\r\n",
        output: [["a", "b\rc", "d"]],
      },
      {
        name: "NoEOLTest",
        input: "a,b,c",
        output: [["a", "b", "c"]],
      },
      {
        name: "Semicolon",
        input: "a;b;c\n",
        output: [["a", "b", "c"]],
        separator: ";",
      },
      {
        name: "MultiLine",
        input: `"two
line","one line","three
line
field"`,
        output: [["two\nline", "one line", "three\nline\nfield"]],
      },
      {
        name: "BlankLine",
        input: "a,b,c\n\nd,e,f\n\n",
        output: [
          ["a", "b", "c"],
          ["d", "e", "f"],
        ],
      },
      {
        name: "LeadingSpace",
        input: " a,  b,   c\n",
        output: [[" a", "  b", "   c"]],
      },
      {
        name: "Comment",
        input: "#1,2,3\na,b,c\n#comment",
        output: [["a", "b", "c"]],
        comment: "#",
      },
      {
        name: "NoComment",
        input: "#1,2,3\na,b,c",
        output: [
          ["#1", "2", "3"],
          ["a", "b", "c"],
        ],
      },
      {
        name: "FieldCount",
        input: "a,b,c\nd,e",
        output: [
          ["a", "b", "c"],
          ["d", "e"],
        ],
      },
      {
        name: "TrailingCommaEOF",
        input: "a,b,c,",
        output: [["a", "b", "c", ""]],
      },
      {
        name: "TrailingCommaEOL",
        input: "a,b,c,\n",
        output: [["a", "b", "c", ""]],
      },
      {
        name: "NotTrailingComma3",
        input: "a,b,c, \n",
        output: [["a", "b", "c", " "]],
      },
      {
        name: "CommaFieldTest",
        input: `x,y,z,w
x,y,z,
x,y,,
x,,,
,,,
"x","y","z","w"
"x","y","z",""
"x","y","",""
"x","","",""
"","","",""
`,
        output: [
          ["x", "y", "z", "w"],
          ["x", "y", "z", ""],
          ["x", "y", "", ""],
          ["x", "", "", ""],
          ["", "", "", ""],
          ["x", "y", "z", "w"],
          ["x", "y", "z", ""],
          ["x", "y", "", ""],
          ["x", "", "", ""],
          ["", "", "", ""],
        ],
      },
      {
        name: "CRLFInQuotedField", // Issue 21201
        input: 'A,"Hello\r\nHi",B\r\n',
        output: [["A", "Hello\nHi", "B"]],
      },
      {
        name: "BinaryBlobField", // Issue 19410
        input: "x09\x41\xb4\x1c,aktau",
        output: [["x09A\xb4\x1c", "aktau"]],
      },
      {
        name: "TrailingCR",
        input: "field1,field2\r",
        output: [["field1", "field2"]],
      },
      {
        name: "QuotedTrailingCR",
        input: '"field"\r',
        output: [["field"]],
      },
      {
        name: "FieldCR",
        input: "field\rfield\r",
        output: [["field\rfield"]],
      },
      {
        name: "FieldCRCR",
        input: "field\r\rfield\r\r",
        output: [["field\r\rfield\r"]],
      },
      {
        name: "FieldCRCRLF",
        input: "field\r\r\nfield\r\r\n",
        output: [["field\r"], ["field\r"]],
      },
      {
        name: "FieldCRCRLFCR",
        input: "field\r\r\n\rfield\r\r\n\r",
        output: [["field\r"], ["\rfield\r"]],
      },
      {
        name: "MultiFieldCRCRLFCRCR",
        input: "field1,field2\r\r\n\r\rfield1,field2\r\r\n\r\r,",
        output: [
          ["field1", "field2\r"],
          ["\r\rfield1", "field2\r"],
          ["\r\r", ""],
        ],
      },
      {
        name: "NonASCIICommaAndCommentWithQuotes",
        input: 'a€"  b,"€ c\nλ comment\n',
        output: [["a", "  b,", " c"]],
        separator: "€",
        comment: "λ",
      },
      {
        // λ and θ start with the same byte.
        // This tests that the parser doesn't confuse such characters.
        name: "NonASCIICommaConfusion",
        input: '"abθcd"λefθgh',
        output: [["abθcd", "efθgh"]],
        separator: "λ",
        comment: "€",
      },
      {
        name: "NonASCIICommentConfusion",
        input: "λ\nλ\nθ\nλ\n",
        output: [["λ"], ["λ"], ["λ"]],
        comment: "θ",
      },
      {
        name: "QuotedFieldMultipleLF",
        input: '"\n\n\n\n"',
        output: [["\n\n\n\n"]],
      },
      {
        name: "MultipleCRLF",
        input: "\r\n\r\n\r\n\r\n",
        output: [],
      },
      {
        name: "DoubleQuoteWithTrailingCRLF",
        input: '"foo""bar"\r\n',
        output: [[`foo"bar`]],
      },
      {
        name: "EvenQuotes",
        input: `""""""""`,
        output: [[`"""`]],
      },
      {
        name: "simple",
        input: "a,b,c",
        output: [["a", "b", "c"]],
        skipFirstRow: false,
      },
      {
        name: "multiline",
        input: "a,b,c\ne,f,g\n",
        output: [
          ["a", "b", "c"],
          ["e", "f", "g"],
        ],
        skipFirstRow: false,
      },
      {
        name: "header mapping boolean",
        input: "a,b,c\ne,f,g\n",
        output: [{ a: "e", b: "f", c: "g" }],
        skipFirstRow: true,
      },
      {
        name: "header mapping array",
        input: "a,b,c\ne,f,g\n",
        output: [
          { this: "a", is: "b", sparta: "c" },
          { this: "e", is: "f", sparta: "g" },
        ],
        columns: ["this", "is", "sparta"],
      },
      {
        name: "provides both opts.skipFirstRow and opts.columns",
        input: "a,b,1\nc,d,2\ne,f,3",
        output: [
          { foo: "c", bar: "d", baz: "2" },
          { foo: "e", bar: "f", baz: "3" },
        ],
        skipFirstRow: true,
        columns: ["foo", "bar", "baz"],
      },
      {
        name: "mismatching number of headers and fields",
        input: "a,b,c\nd,e",
        skipFirstRow: true,
        columns: ["foo", "bar", "baz"],
        errorMessage:
          "Error number of fields line: 1\nNumber of fields found: 3\nExpected number of fields: 2",
      },
    ];
    for (const testCase of testCases) {
      await t.step(testCase.name, async () => {
        const options: CsvParseStreamOptions = {};
        if (testCase.separator) {
          options.separator = testCase.separator;
        }
        if (testCase.comment) {
          options.comment = testCase.comment;
        }
        if (testCase.skipFirstRow) {
          options.skipFirstRow = testCase.skipFirstRow;
        }
        if (testCase.columns) {
          options.columns = testCase.columns;
        }
        const readable = ReadableStream.from(testCase.input)
          .pipeThrough(new CsvParseStream(options));

        if (testCase.output) {
          const actual = await Array.fromAsync(readable);
          assertEquals(actual, testCase.output);
        } else {
          await assertRejects(async () => {
            for await (const _ of readable);
          }, testCase.errorMessage);
        }
      });
    }
  },
});

Deno.test({
  name:
    "[csv/csv_parse_stream] cancel CsvParseStream during iteration does not leak file",
  permissions: { read: [testdataDir] },
  fn: async () => {
    const file = await Deno.open(join(testdataDir, "large.csv"));
    const readable = file.readable
      .pipeThrough(new TextDecoderStream())
      .pipeThrough(new CsvParseStream());
    for await (const _record of readable) {
      break;
    }
    // FIXME(kt3k): Remove this delay.
    await delay(100);
  },
});

Deno.test({
  name: "[csv/csv_parse_stream] correct typing",
  fn() {
    // If no option is passed, defaults to ReadableStream<string[]>.
    {
      const { readable } = new CsvParseStream();
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }
    {
      const { readable } = new CsvParseStream(undefined);
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }
    {
      // `skipFirstRow` may be `true` or `false`.
      // `columns` may be `undefined` or `string[]`.
      // If you don't know exactly what the value of the option is,
      // the return type is ReadableStream<string[] | Record<string, string | undefined>>
      const options: CsvParseStreamOptions = {};
      const { readable } = new CsvParseStream(options);
      type _ = AssertTrue<
        IsExact<
          typeof readable,
          ReadableStream<string[] | Record<string, string | undefined>>
        >
      >;
    }
    {
      const { readable } = new CsvParseStream({});
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }

    // skipFirstRow option
    {
      const { readable } = new CsvParseStream({ skipFirstRow: undefined });
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }
    {
      const { readable } = new CsvParseStream({ skipFirstRow: false });
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }
    {
      const { readable } = new CsvParseStream({ skipFirstRow: true });
      type _ = AssertTrue<
        IsExact<
          typeof readable,
          ReadableStream<Record<string, string | undefined>>
        >
      >;
    }

    // columns option
    {
      const { readable } = new CsvParseStream({ columns: undefined });
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }
    {
      const { readable } = new CsvParseStream({ columns: ["aaa", "bbb"] });
      type _ = AssertTrue<
        IsExact<typeof readable, ReadableStream<Record<"aaa" | "bbb", string>>>
      >;
    }
    {
      const { readable } = new CsvParseStream({ columns: ["aaa"] as string[] });
      type _ = AssertTrue<
        IsExact<
          typeof readable,
          ReadableStream<Record<string, string | undefined>>
        >
      >;
    }

    // skipFirstRow option + columns option
    {
      const { readable } = new CsvParseStream({
        skipFirstRow: false,
        columns: undefined,
      });
      type _ = AssertTrue<IsExact<typeof readable, ReadableStream<string[]>>>;
    }
    {
      const { readable } = new CsvParseStream({
        skipFirstRow: true,
        columns: undefined,
      });
      type _ = AssertTrue<
        IsExact<
          typeof readable,
          ReadableStream<Record<string, string | undefined>>
        >
      >;
    }
    {
      const { readable } = new CsvParseStream({
        skipFirstRow: false,
        columns: ["aaa"],
      });
      type _ = AssertTrue<
        IsExact<typeof readable, ReadableStream<Record<"aaa", string>>>
      >;
    }
    {
      const { readable } = new CsvParseStream({
        skipFirstRow: true,
        columns: ["aaa"],
      });
      type _ = AssertTrue<
        IsExact<typeof readable, ReadableStream<Record<"aaa", string>>>
      >;
    }
  },
});
