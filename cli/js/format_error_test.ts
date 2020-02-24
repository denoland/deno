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

function ansiRegex(): RegExp {
  return new RegExp(
    [
      "[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:[a-zA-Z\\d]*(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)",
      "(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))"
    ].join("|"),
    "g"
  );
}

function stripAnsi(input: string): string {
  return input.replace(ansiRegex(), "");
}

const expected01 = `error TS4000: Example error

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ^
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

const expected02 = `error TS4000: Example error

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~
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

const expected03 = `error TS4000: First level
Level 2 01
Level 2 02
  Level 3 01

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
    ~~~~
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

const expected04 = `error TS4000: Example error

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~

  Related information

    ► foo.ts:100:4

    100 1234567890
           ~~~~
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

const expected05 = `error TS4000: Error 001

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~

error TS4001: Error 002

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~

error TS4002: Error 003

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~

error TS4003: Error 004

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~


Found 4 errors.
`;

test(function formatDiagnosticBasic() {
  const actual = formatDiagnostics(fixture01);
  assertEquals(stripAnsi(actual), expected01);
});

test(function formatDiagnosticColSpan() {
  const actual = formatDiagnostics(fixture02);
  assertEquals(stripAnsi(actual), expected02);
});

test(function formatDiagnosticMessageChain() {
  const actual = formatDiagnostics(fixture03);
  assertEquals(stripAnsi(actual), expected03);
});

test(function formatDiagnosticRelatedInfo() {
  const actual = formatDiagnostics(fixture04);
  assertEquals(stripAnsi(actual), expected04);
});

test(function formatDiagnosticRelatedInfo() {
  const actual = formatDiagnostics(fixture05);
  assertEquals(stripAnsi(actual), expected05);
});
