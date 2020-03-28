// Test ported from Golang
// https://github.com/golang/go/blob/2cc15b1/src/encoding/csv/reader_test.go
import { assertEquals, assert } from "../testing/asserts.ts";
import { readMatrix, parse } from "./csv.ts";
import { StringReader } from "../io/readers.ts";
import { BufReader } from "../io/bufio.ts";

const ErrInvalidDelim = "Invalid Delimiter";
const ErrFieldCount = "wrong number of fields";
const ErrBareQuote = 'bare " in non-quoted-field';

// TODO(zekth): Activate remaining tests
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
    ignore: true,
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
    Output: [["two\nline"], ["one line"], ["three\nline\nfield"]],
    ignore: true,
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
    Error: ErrBareQuote,
    // Error: &ParseError{StartLine: 1, Line: 1, Column: 1, Err: ErrBareQuote},
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
    Error: ErrBareQuote,
    // &ParseError{StartLine: 1, Line: 1, Column: 2, Err: ErrBareQuote}
  },
  {
    Name: "BadTrailingQuote",
    Input: `"a word",b"`,
    Error: ErrBareQuote,
  },
  {
    Name: "ExtraneousQuote",
    Input: `"a "word","b"`,
    Error: ErrBareQuote,
  },
  {
    Name: "BadFieldCount",
    Input: "a,b,c\nd,e",
    Error: ErrFieldCount,
    UseFieldsPerRecord: true,
    FieldsPerRecord: 0,
  },
  {
    Name: "BadFieldCount1",
    Input: `a,b,c`,
    // Error: &ParseError{StartLine: 1, Line: 1, Err: ErrFieldCount},
    UseFieldsPerRecord: true,
    FieldsPerRecord: 2,
    Error: ErrFieldCount,
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
    Error: true,
    // Error: &ParseError{StartLine: 1, Line: 2, Column: 1, Err: ErrQuote},
    ignore: true,
  },
  {
    Name: "StartLine2",
    Input: 'a,b\n"d\n\n,e',
    Error: true,
    // Error: &ParseError{StartLine: 2, Line: 5, Column: 0, Err: ErrQuote},
    ignore: true,
  },
  {
    Name: "CRLFInQuotedField", // Issue 21201
    Input: 'A,"Hello\r\nHi",B\r\n',
    Output: [["A", "Hello\nHi", "B"]],
    ignore: true,
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
    ignore: true,
  },
  {
    Name: "QuotedTrailingCR",
    Input: '"field"\r',
    Output: [['"field"']],
    ignore: true,
  },
  {
    Name: "QuotedTrailingCRCR",
    Input: '"field"\r\r',
    Error: true,
    // Error: &ParseError{StartLine: 1, Line: 1, Column: 6, Err: ErrQuote},
    ignore: true,
  },
  {
    Name: "FieldCR",
    Input: "field\rfield\r",
    Output: [["field\rfield"]],
    ignore: true,
  },
  {
    Name: "FieldCRCR",
    Input: "field\r\rfield\r\r",
    Output: [["field\r\rfield\r"]],
    ignore: true,
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
    ignore: true,
  },
  {
    Name: "MultiFieldCRCRLFCRCR",
    Input: "field1,field2\r\r\n\r\rfield1,field2\r\r\n\r\r,",
    Output: [
      ["field1", "field2\r"],
      ["\r\rfield1", "field2\r"],
      ["\r\r", ""],
    ],
    ignore: true,
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
    ignore: true,
  },
  {
    Name: "MultipleCRLF",
    Input: "\r\n\r\n\r\n\r\n",
    ignore: true,
  },
  /**
   * The implementation may read each line in several chunks if
   * it doesn't fit entirely.
   * in the read buffer, so we should test the code to handle that condition.
   */
  {
    Name: "HugeLines",
    Input:
      "#ignore\n".repeat(10000) + "@".repeat(5000) + "," + "*".repeat(5000),
    Output: [["@".repeat(5000), "*".repeat(5000)]],
    Comment: "#",
    ignore: true,
  },
  {
    Name: "QuoteWithTrailingCRLF",
    Input: '"foo"bar"\r\n',
    Error: ErrBareQuote,
    // Error: &ParseError{StartLine: 1, Line: 1, Column: 4, Err: ErrQuote},
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
    ignore: true,
  },
  {
    Name: "EvenQuotes",
    Input: `""""""""`,
    Output: [[`"""`]],
    ignore: true,
  },
  {
    Name: "OddQuotes",
    Input: `"""""""`,
    Error: true,
    // Error:" &ParseError{StartLine: 1, Line: 1, Column: 7, Err: ErrQuote}",
    ignore: true,
  },
  {
    Name: "LazyOddQuotes",
    Input: `"""""""`,
    Output: [[`"""`]],
    LazyQuotes: true,
    ignore: true,
  },
  {
    Name: "BadComma1",
    Comma: "\n",
    Error: ErrInvalidDelim,
  },
  {
    Name: "BadComma2",
    Comma: "\r",
    Error: ErrInvalidDelim,
  },
  {
    Name: "BadComma3",
    Comma: '"',
    Error: ErrInvalidDelim,
  },
  {
    Name: "BadComment1",
    Comment: "\n",
    Error: ErrInvalidDelim,
  },
  {
    Name: "BadComment2",
    Comment: "\r",
    Error: ErrInvalidDelim,
  },
  {
    Name: "BadCommaComment",
    Comma: "X",
    Comment: "X",
    Error: ErrInvalidDelim,
  },
];
for (const t of testCases) {
  Deno.test({
    ignore: !!t.ignore,
    name: `[CSV] ${t.Name}`,
    async fn(): Promise<void> {
      let comma = ",";
      let comment;
      let fieldsPerRec;
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
        let err;
        try {
          actual = await readMatrix(
            new BufReader(new StringReader(t.Input ?? "")),
            {
              comma: comma,
              comment: comment,
              trimLeadingSpace: trim,
              fieldsPerRecord: fieldsPerRec,
              lazyQuotes: lazyquote,
            }
          );
        } catch (e) {
          err = e;
        }
        assert(err);
        assertEquals(err.message, t.Error);
      } else {
        actual = await readMatrix(
          new BufReader(new StringReader(t.Input ?? "")),
          {
            comma: comma,
            comment: comment,
            trimLeadingSpace: trim,
            fieldsPerRecord: fieldsPerRec,
            lazyQuotes: lazyquote,
          }
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
