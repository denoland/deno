// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { setColorEnabled } from "./colors.ts";
import { formatDiagnostic } from "./diagnostics.ts";
import { assertEquals } from "../testing/asserts.ts";

const { test } = Deno;

const fixture_01: Deno.DiagnosticItem[] = [
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

const expected_01 = `error TS4000: Example error

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ^
`;

const fixture_02: Deno.DiagnosticItem[] = [
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

const expected_02 = `error TS4000: Example error

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~
`;

const fixture_03: Deno.DiagnosticItem[] = [
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

const expected_03 = `error TS4000: First level
  Level 2 01
  Level 2 02
    Level 3 01

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~
`;

const fixture_04: Deno.DiagnosticItem[] = [
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

const expected_04 = `error TS4000: Example error

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~

  Related information

    ► foo.ts:100:4

    100 1234567890
           ~~~~

`;

const fixture_05: Deno.DiagnosticItem[] = [
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

const expected_05 = `error TS4000: Error 001

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
`;

const expected_05_limit = `error TS4000: Error 001

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~
error TS4001: Error 002

► foo.ts:1001:2

1001 abcdefghijklmnopqrstuv
      ~~~~


Additional 2 item(s) found.
`;

setColorEnabled(false);

test(function diagnosticsBasic() {
  const actual = formatDiagnostic(fixture_01);
  assertEquals(actual, expected_01);
});

test(function diagnosticsColumnSpan() {
  const actual = formatDiagnostic(fixture_02);
  assertEquals(actual, expected_02);
});

test(function diagnosticsMessageChain() {
  const actual = formatDiagnostic(fixture_03);
  assertEquals(actual, expected_03);
});

test(function diagnosticsRelatedInfo() {
  const actual = formatDiagnostic(fixture_04);
  assertEquals(actual, expected_04);
});

test(function diagnosticsMultiple() {
  const actual = formatDiagnostic(fixture_05);
  assertEquals(actual, expected_05);
});

test(function diagnosticsLimit() {
  const actual = formatDiagnostic(fixture_05, { limit: 2 });
  assertEquals(actual, expected_05_limit);
});
