// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test("formatDiagnosticBasic", function () {
  const fixture: Deno.Diagnostic[] = [
    {
      start: {
        line: 0,
        character: 0,
      },
      end: {
        line: 0,
        character: 7,
      },
      fileName: "test.ts",
      messageText:
        "Cannot find name 'console'. Do you need to change your target library? Try changing the `lib` compiler option to include 'dom'.",
      sourceLine: `console.log("a");`,
      category: 1,
      code: 2584,
    },
  ];
  const out = Deno.formatDiagnostics(fixture);
  assert(out.includes("Cannot find name"));
  assert(out.includes("test.ts"));
});

Deno.test("formatDiagnosticError", function () {
  let thrown = false;
  // deno-lint-ignore no-explicit-any
  const bad = ([{ hello: 123 }] as any) as Deno.Diagnostic[];
  try {
    Deno.formatDiagnostics(bad);
  } catch (e) {
    assert(e instanceof Deno.errors.InvalidData);
    thrown = true;
  }
  assert(thrown);
});
