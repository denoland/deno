// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals, test } from "./test_util.ts";

const { formatDiagnostics } = Deno;

const fixture01: Deno.DiagnosticItem[] = [
  {
    message: "Example error",
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 2,
    code: 4000
  }
];

const expected01 = `[1;31merror[0m[1m TS4000[0m: Example error

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ^[0m
`;

const fixture02: Deno.DiagnosticItem[] = [
  {
    message: "Example error",
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4000
  }
];

const expected02 = `[1;31merror[0m[1m TS4000[0m: Example error

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m
`;

const fixture03: Deno.DiagnosticItem[] = [
  {
    message: "Example error",
    messageChain: {
      message: "First level",
      category: Deno.DiagnosticCategory.Error,
      code: 4001,
      next: [
        {
          message: "Level 2 01",
          category: Deno.DiagnosticCategory.Error,
          code: 4002
        },
        {
          message: "Level 2 02",
          category: Deno.DiagnosticCategory.Error,
          code: 4003,
          next: [
            {
              message: "Level 3 01",
              category: Deno.DiagnosticCategory.Error,
              code: 4004
            }
          ]
        }
      ]
    },
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4000
  }
];

const expected03 = `[1;31merror[0m[1m TS4000[0m: First level
  Level 2 01
  Level 2 02
    Level 3 01

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m
`;

const fixture04: Deno.DiagnosticItem[] = [
  {
    message: "Example error",
    relatedInformation: [
      {
        message: "Related information",
        category: Deno.DiagnosticCategory.Info,
        sourceLine: "1234567890",
        lineNumber: 99,
        scriptResourceName: "foo.ts",
        startColumn: 3,
        endColumn: 7,
        code: 1000
      }
    ],
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4000
  }
];

const expected04 = `[1;31merror[0m[1m TS4000[0m: Example error

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m

  Related information

    ► [38;5;14mfoo.ts[0m:[38;5;11m100[0m:[38;5;11m4[0m

    [47;30m100[0m 1234567890
    [47;30m   [0m [38;5;14m   ~~~~[0m

`;

const fixture05: Deno.DiagnosticItem[] = [
  {
    message: "Error 001",
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4000
  },
  {
    message: "Error 002",
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4001
  },
  {
    message: "Error 003",
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4002
  },
  {
    message: "Error 004",
    category: Deno.DiagnosticCategory.Error,
    sourceLine: "abcdefghijklmnopqrstuv",
    lineNumber: 1000,
    scriptResourceName: "foo.ts",
    startColumn: 1,
    endColumn: 5,
    code: 4003
  }
];

const expected05 = `[1;31merror[0m[1m TS4000[0m: Error 001

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m

[1;31merror[0m[1m TS4001[0m: Error 002

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m

[1;31merror[0m[1m TS4002[0m: Error 003

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m

[1;31merror[0m[1m TS4003[0m: Error 004

► [38;5;14mfoo.ts[0m:[38;5;11m1001[0m:[38;5;11m2[0m

[47;30m1001[0m abcdefghijklmnopqrstuv
[47;30m    [0m [31m ~~~~[0m


Found 4 errors.
`;

test(function formatDiagnosticBasic() {
  const actual = formatDiagnostics(fixture01);
  assertEquals(actual, expected01);
});

test(function formatDiagnosticColSpan() {
  const actual = formatDiagnostics(fixture02);
  assertEquals(actual, expected02);
});

test(function formatDiagnosticMessageChain() {
  const actual = formatDiagnostics(fixture03);
  assertEquals(actual, expected03);
});

test(function formatDiagnosticRelatedInfo() {
  const actual = formatDiagnostics(fixture04);
  assertEquals(actual, expected04);
});

test(function formatDiagnosticRelatedInfo() {
  const actual = formatDiagnostics(fixture05);
  assertEquals(actual, expected05);
});
