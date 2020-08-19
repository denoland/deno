// Test ported from Golang
// https://github.com/golang/go/blob/2cc15b1/src/encoding/csv/reader_test.go
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
import {
  readMatrix,
  parse,
  ERR_BARE_QUOTE,
  ERR_QUOTE,
  ERR_INVALID_DELIM,
  ERR_FIELD_COUNT,
  ParseError,
} from "./csv.ts";
import { StringReader } from "../io/readers.ts";
import { BufReader } from "../io/bufio.ts";

const testCases = [
  {
    Name: "Simple",
    Input: "a,b,c\n",
    Output: [["a", "b", "c"]],
  },
  {
    Name: "CRLF",
    Input: "a,b\r\nc,d\r\n",
    Output: [
      ["a", "b"],
      ["c", "d"],
    ],
  },
  {
    Name: "BareCR",
    Input: "a,b\rc,d\r\n",
    Output: [["a", "b\rc", "d"]],
  },
  {
    Name: "RFC4180test",
    Input: `#field1,field2,field3
"aaa","bbb","ccc"
"a,a","bbb","ccc"
zzz,yyy,xxx`,
    UseFieldsPerRecord: true,
    FieldsPerRecord: 0,
    Output: [
      ["#field1", "field2", "field3"],
      ["aaa", "bbb", "ccc"],
      ["a,a", `bbb`, "ccc"],
      ["zzz", "yyy", "xxx"],
    ],
  },
  {
    Name: "NoEOLTest",
    Input: "a,b,c",
    Output: [["a", "b", "c"]],
  },
  {
    Name: "Semicolon",
    Input: "a;b;c\n",
    Output: [["a", "b", "c"]],
    Comma: ";",
  },
  {
    Name: "MultiLine",
    Input: `"two
line","one line","three
line
field"`,
    Output: [["two\nline", "one line", "three\nline\nfield"]],
  },
  {
    Name: "BlankLine",
    Input: "a,b,c\n\nd,e,f\n\n",
    Output: [
      ["a", "b", "c"],
      ["d", "e", "f"],
    ],
  },
  {
    Name: "BlankLineFieldCount",
    Input: "a,b,c\n\nd,e,f\n\n",
    Output: [
      ["a", "b", "c"],
      ["d", "e", "f"],
    ],
    UseFieldsPerRecord: true,
    FieldsPerRecord: 0,
  },
  {
    Name: "TrimSpace",
    Input: " a,  b,   c\n",
    Output: [["a", "b", "c"]],
    TrimLeadingSpace: true,
  },
  {
    Name: "LeadingSpace",
    Input: " a,  b,   c\n",
    Output: [[" a", "  b", "   c"]],
  },
  {
    Name: "Comment",
    Input: "#1,2,3\na,b,c\n#comment",
    Output: [["a", "b", "c"]],
    Comment: "#",
  },
  {
    Name: "NoComment",
    Input: "#1,2,3\na,b,c",
    Output: [
      ["#1", "2", "3"],
      ["a", "b", "c"],
    ],
  },
  {
    Name: "LazyQuotes",
    Input: `a "word","1"2",a","b`,
    Output: [[`a "word"`, `1"2`, `a"`, `b`]],
    LazyQuotes: true,
  },
  {
    Name: "BareQuotes",
    Input: `a "word","1"2",a"`,
    Output: [[`a "word"`, `1"2`, `a"`]],
    LazyQuotes: true,
  },
  {
    Name: "BareDoubleQuotes",
    Input: `a""b,c`,
    Output: [[`a""b`, `c`]],
    LazyQuotes: true,
  },
  {
    Name: "BadDoubleQuotes",
    Input: `a""b,c`,
    Error: new ParseError(1, 1, 1, ERR_BARE_QUOTE),
  },
  {
    Name: "TrimQuote",
    Input: ` "a"," b",c`,
    Output: [["a", " b", "c"]],
    TrimLeadingSpace: true,
  },
  {
    Name: "BadBareQuote",
    Input: `a "word","b"`,
    Error: new ParseError(1, 1, 2, ERR_BARE_QUOTE),
  },
  {
    Name: "BadTrailingQuote",
    Input: `"a word",b"`,
    Error: new ParseError(1, 1, 10, ERR_BARE_QUOTE),
  },
  {
    Name: "ExtraneousQuote",
    Input: `"a "word","b"`,
    Error: new ParseError(1, 1, 3, ERR_QUOTE),
  },
  {
    Name: "BadFieldCount",
    Input: "a,b,c\nd,e",
    Error: new ParseError(2, 2, null, ERR_FIELD_COUNT),
    UseFieldsPerRecord: true,
    FieldsPerRecord: 0,
  },
  {
    Name: "BadFieldCount1",
    Input: `a,b,c`,
    UseFieldsPerRecord: true,
    FieldsPerRecord: 2,
    Error: new ParseError(1, 1, null, ERR_FIELD_COUNT),
  },
  {
    Name: "FieldCount",
    Input: "a,b,c\nd,e",
    Output: [
      ["a", "b", "c"],
      ["d", "e"],
    ],
  },
  {
    Name: "TrailingCommaEOF",
    Input: "a,b,c,",
    Output: [["a", "b", "c", ""]],
  },
  {
    Name: "TrailingCommaEOL",
    Input: "a,b,c,\n",
    Output: [["a", "b", "c", ""]],
  },
  {
    Name: "TrailingCommaSpaceEOF",
    Input: "a,b,c, ",
    Output: [["a", "b", "c", ""]],
    TrimLeadingSpace: true,
  },
  {
    Name: "TrailingCommaSpaceEOL",
    Input: "a,b,c, \n",
    Output: [["a", "b", "c", ""]],
    TrimLeadingSpace: true,
  },
  {
    Name: "TrailingCommaLine3",
    Input: "a,b,c\nd,e,f\ng,hi,",
    Output: [
      ["a", "b", "c"],
      ["d", "e", "f"],
      ["g", "hi", ""],
    ],
    TrimLeadingSpace: true,
  },
  {
    Name: "NotTrailingComma3",
    Input: "a,b,c, \n",
    Output: [["a", "b", "c", " "]],
  },
  {
    Name: "CommaFieldTest",
    Input: `x,y,z,w
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
    Output: [
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
    Name: "TrailingCommaIneffective1",
    Input: "a,b,\nc,d,e",
    Output: [
      ["a", "b", ""],
      ["c", "d", "e"],
    ],
    TrimLeadingSpace: true,
  },
  {
    Name: "ReadAllReuseRecord",
    Input: "a,b\nc,d",
    Output: [
      ["a", "b"],
      ["c", "d"],
    ],
    ReuseRecord: true,
  },
  {
    Name: "StartLine1", // Issue 19019
    Input: 'a,"b\nc"d,e',
    Error: new ParseError(1, 2, 1, ERR_QUOTE),
  },
  {
    Name: "StartLine2",
    Input: 'a,b\n\"d\n\n,e',
    Error: new ParseError(2, 5, 0, ERR_QUOTE),
  },
  {
    Name: "CRLFInQuotedField", // Issue 21201
    Input: 'A,"Hello\r\nHi",B\r\n',
    Output: [["A", "Hello\nHi", "B"]],
  },
  {
    Name: "BinaryBlobField", // Issue 19410
    Input: "x09\x41\xb4\x1c,aktau",
    Output: [["x09A\xb4\x1c", "aktau"]],
  },
  {
    Name: "TrailingCR",
    Input: "field1,field2\r",
    Output: [["field1", "field2"]],
  },
  {
    Name: "QuotedTrailingCR",
    Input: '"field"\r',
    Output: [["field"]],
  },
  {
    Name: "QuotedTrailingCRCR",
    Input: '"field"\r\r',
    Error: new ParseError(1, 1, 6, ERR_QUOTE),
  },
  {
    Name: "FieldCR",
    Input: "field\rfield\r",
    Output: [["field\rfield"]],
  },
  {
    Name: "FieldCRCR",
    Input: "field\r\rfield\r\r",
    Output: [["field\r\rfield\r"]],
  },
  {
    Name: "FieldCRCRLF",
    Input: "field\r\r\nfield\r\r\n",
    Output: [["field\r"], ["field\r"]],
  },
  {
    Name: "FieldCRCRLFCR",
    Input: "field\r\r\n\rfield\r\r\n\r",
    Output: [["field\r"], ["\rfield\r"]],
  },
  {
    Name: "FieldCRCRLFCRCR",
    Input: "field\r\r\n\r\rfield\r\r\n\r\r",
    Output: [["field\r"], ["\r\rfield\r"], ["\r"]],
  },
  {
    Name: "MultiFieldCRCRLFCRCR",
    Input: "field1,field2\r\r\n\r\rfield1,field2\r\r\n\r\r,",
    Output: [
      ["field1", "field2\r"],
      ["\r\rfield1", "field2\r"],
      ["\r\r", ""],
    ],
  },
  {
    Name: "NonASCIICommaAndComment",
    Input: "a£b,c£ \td,e\n€ comment\n",
    Output: [["a", "b,c", "d,e"]],
    TrimLeadingSpace: true,
    Comma: "£",
    Comment: "€",
  },
  {
    Name: "NonASCIICommaAndCommentWithQuotes",
    Input: 'a€"  b,"€ c\nλ comment\n',
    Output: [["a", "  b,", " c"]],
    Comma: "€",
    Comment: "λ",
  },
  {
    // λ and θ start with the same byte.
    // This tests that the parser doesn't confuse such characters.
    Name: "NonASCIICommaConfusion",
    Input: '"abθcd"λefθgh',
    Output: [["abθcd", "efθgh"]],
    Comma: "λ",
    Comment: "€",
  },
  {
    Name: "NonASCIICommentConfusion",
    Input: "λ\nλ\nθ\nλ\n",
    Output: [["λ"], ["λ"], ["λ"]],
    Comment: "θ",
  },
  {
    Name: "QuotedFieldMultipleLF",
    Input: '"\n\n\n\n"',
    Output: [["\n\n\n\n"]],
  },
  {
    Name: "MultipleCRLF",
    Input: "\r\n\r\n\r\n\r\n",
    Output: [],
  },
  /**
   * The implementation may read each line in several chunks if
   * it doesn't fit entirely.
   * in the read buffer, so we should test the code to handle that condition.
   */
  {
    Name: "HugeLines",
    Input: "#ignore\n".repeat(10000) + "@".repeat(5000) + "," +
      "*".repeat(5000),
    Output: [["@".repeat(5000), "*".repeat(5000)]],
    Comment: "#",
  },
  {
    Name: "QuoteWithTrailingCRLF",
    Input: '"foo"bar"\r\n',
    Error: new ParseError(1, 1, 4, ERR_QUOTE),
  },
  {
    Name: "LazyQuoteWithTrailingCRLF",
    Input: '"foo"bar"\r\n',
    Output: [[`foo"bar`]],
    LazyQuotes: true,
  },
  {
    Name: "DoubleQuoteWithTrailingCRLF",
    Input: '"foo""bar"\r\n',
    Output: [[`foo"bar`]],
  },
  {
    Name: "EvenQuotes",
    Input: `""""""""`,
    Output: [[`"""`]],
  },
  {
    Name: "OddQuotes",
    Input: `"""""""`,
    Error: new ParseError(1, 1, 7, ERR_QUOTE),
  },
  {
    Name: "LazyOddQuotes",
    Input: `"""""""`,
    Output: [[`"""`]],
    LazyQuotes: true,
  },
  {
    Name: "BadComma1",
    Comma: "\n",
    Error: new Error(ERR_INVALID_DELIM),
  },
  {
    Name: "BadComma2",
    Comma: "\r",
    Error: new Error(ERR_INVALID_DELIM),
  },
  {
    Name: "BadComma3",
    Comma: '"',
    Error: new Error(ERR_INVALID_DELIM),
  },
  {
    Name: "BadComment1",
    Comment: "\n",
    Error: new Error(ERR_INVALID_DELIM),
  },
  {
    Name: "BadComment2",
    Comment: "\r",
    Error: new Error(ERR_INVALID_DELIM),
  },
  {
    Name: "BadCommaComment",
    Comma: "X",
    Comment: "X",
    Error: new Error(ERR_INVALID_DELIM),
  },
];
for (const t of testCases) {
  Deno.test({
    name: `[CSV] ${t.Name}`,
    async fn(): Promise<void> {
      let comma = ",";
      let comment: string | undefined;
      let fieldsPerRec: number | undefined;
      let trim = false;
      let lazyquote = false;
      if (t.Comma) {
        comma = t.Comma;
      }
      if (t.Comment) {
        comment = t.Comment;
      }
      if (t.TrimLeadingSpace) {
        trim = true;
      }
      if (t.UseFieldsPerRecord) {
        fieldsPerRec = t.FieldsPerRecord;
      }
      if (t.LazyQuotes) {
        lazyquote = t.LazyQuotes;
      }
      let actual;
      if (t.Error) {
        const err = await assertThrowsAsync(async () => {
          await readMatrix(
            new BufReader(new StringReader(t.Input ?? "")),
            {
              comma: comma,
              comment: comment,
              trimLeadingSpace: trim,
              fieldsPerRecord: fieldsPerRec,
              lazyQuotes: lazyquote,
            },
          );
        });

        assertEquals(err, t.Error);
      } else {
        actual = await readMatrix(
          new BufReader(new StringReader(t.Input ?? "")),
          {
            comma: comma,
            comment: comment,
            trimLeadingSpace: trim,
            fieldsPerRecord: fieldsPerRec,
            lazyQuotes: lazyquote,
          },
        );
        const expected = t.Output;
        assertEquals(actual, expected);
      }
    },
  });
}

const parseTestCases = [
  {
    name: "simple",
    in: "a,b,c",
    header: false,
    result: [["a", "b", "c"]],
  },
  {
    name: "simple Bufreader",
    in: new BufReader(new StringReader("a,b,c")),
    header: false,
    result: [["a", "b", "c"]],
  },
  {
    name: "multiline",
    in: "a,b,c\ne,f,g\n",
    header: false,
    result: [
      ["a", "b", "c"],
      ["e", "f", "g"],
    ],
  },
  {
    name: "header mapping boolean",
    in: "a,b,c\ne,f,g\n",
    header: true,
    result: [{ a: "e", b: "f", c: "g" }],
  },
  {
    name: "header mapping array",
    in: "a,b,c\ne,f,g\n",
    header: ["this", "is", "sparta"],
    result: [
      { this: "a", is: "b", sparta: "c" },
      { this: "e", is: "f", sparta: "g" },
    ],
  },
  {
    name: "header mapping object",
    in: "a,b,c\ne,f,g\n",
    header: [{ name: "this" }, { name: "is" }, { name: "sparta" }],
    result: [
      { this: "a", is: "b", sparta: "c" },
      { this: "e", is: "f", sparta: "g" },
    ],
  },
  {
    name: "header mapping parse entry",
    in: "a,b,c\ne,f,g\n",
    header: [
      {
        name: "this",
        parse: (e: string): string => {
          return `b${e}$$`;
        },
      },
      {
        name: "is",
        parse: (e: string): number => {
          return e.length;
        },
      },
      {
        name: "sparta",
        parse: (e: string): unknown => {
          return { bim: `boom-${e}` };
        },
      },
    ],
    result: [
      { this: "ba$$", is: 1, sparta: { bim: `boom-c` } },
      { this: "be$$", is: 1, sparta: { bim: `boom-g` } },
    ],
  },
  {
    name: "multiline parse",
    in: "a,b,c\ne,f,g\n",
    parse: (e: string[]): unknown => {
      return { super: e[0], street: e[1], fighter: e[2] };
    },
    header: false,
    result: [
      { super: "a", street: "b", fighter: "c" },
      { super: "e", street: "f", fighter: "g" },
    ],
  },
  {
    name: "header mapping object parseline",
    in: "a,b,c\ne,f,g\n",
    header: [{ name: "this" }, { name: "is" }, { name: "sparta" }],
    parse: (e: Record<string, unknown>): unknown => {
      return { super: e.this, street: e.is, fighter: e.sparta };
    },
    result: [
      { super: "a", street: "b", fighter: "c" },
      { super: "e", street: "f", fighter: "g" },
    ],
  },
];

for (const testCase of parseTestCases) {
  Deno.test({
    name: `[CSV] Parse ${testCase.name}`,
    async fn(): Promise<void> {
      const r = await parse(testCase.in, {
        header: testCase.header,
        parse: testCase.parse as (input: unknown) => unknown,
      });
      assertEquals(r, testCase.result);
    },
  });
}

Deno.test({
  name: "[CSV] ParseError.message",
  fn(): void {
    assertEquals(
      new ParseError(2, 2, null, ERR_FIELD_COUNT).message,
      `record on line 2: ${ERR_FIELD_COUNT}`,
    );

    assertEquals(
      new ParseError(1, 2, 1, ERR_QUOTE).message,
      `record on line 1; parse error on line 2, column 1: ${ERR_QUOTE}`,
    );

    assertEquals(
      new ParseError(1, 1, 7, ERR_QUOTE).message,
      `parse error on line 1, column 7: ${ERR_QUOTE}`,
    );
  },
});
