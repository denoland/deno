// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertMatch, unitTest } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { setPrepareStackTrace } = Deno[Deno.internal];

interface CallSite {
  getThis(): unknown;
  getTypeName(): string | null;
  // deno-lint-ignore ban-types
  getFunction(): Function | null;
  getFunctionName(): string | null;
  getMethodName(): string | null;
  getFileName(): string | null;
  getLineNumber(): number | null;
  getColumnNumber(): number | null;
  getEvalOrigin(): string | null;
  isToplevel(): boolean | null;
  isEval(): boolean;
  isNative(): boolean;
  isConstructor(): boolean;
  isAsync(): boolean;
  isPromiseAll(): boolean;
  getPromiseIndex(): number | null;
}

function getMockCallSite(
  fileName: string,
  lineNumber: number | null,
  columnNumber: number | null,
): CallSite {
  return {
    getThis(): unknown {
      return undefined;
    },
    getTypeName(): string {
      return "";
    },
    // deno-lint-ignore ban-types
    getFunction(): Function {
      return (): void => {};
    },
    getFunctionName(): string {
      return "";
    },
    getMethodName(): string {
      return "";
    },
    getFileName(): string {
      return fileName;
    },
    getLineNumber(): number | null {
      return lineNumber;
    },
    getColumnNumber(): number | null {
      return columnNumber;
    },
    getEvalOrigin(): null {
      return null;
    },
    isToplevel(): false {
      return false;
    },
    isEval(): false {
      return false;
    },
    isNative(): false {
      return false;
    },
    isConstructor(): false {
      return false;
    },
    isAsync(): false {
      return false;
    },
    isPromiseAll(): false {
      return false;
    },
    getPromiseIndex(): null {
      return null;
    },
  };
}

unitTest(function errorStackMessageLine(): void {
  const e1 = new Error();
  e1.name = "Foo";
  e1.message = "bar";
  assertMatch(e1.stack!, /^Foo: bar\n/);

  const e2 = new Error();
  e2.name = "";
  e2.message = "bar";
  assertMatch(e2.stack!, /^bar\n/);

  const e3 = new Error();
  e3.name = "Foo";
  e3.message = "";
  assertMatch(e3.stack!, /^Foo\n/);

  const e4 = new Error();
  e4.name = "";
  e4.message = "";
  assertMatch(e4.stack!, /^\n/);

  const e5 = new Error();
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e5.name = undefined;
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e5.message = undefined;
  assertMatch(e5.stack!, /^Error\n/);

  const e6 = new Error();
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e6.name = null;
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  e6.message = null;
  assertMatch(e6.stack!, /^null: null\n/);
});

// FIXME(bartlomieju): no longer works after migrating
// to JavaScript runtime code
unitTest({ ignore: true }, function prepareStackTrace(): void {
  // deno-lint-ignore no-explicit-any
  const MockError = {} as any;
  setPrepareStackTrace(MockError);
  assert(typeof MockError.prepareStackTrace === "function");
  const prepareStackTrace: (
    error: Error,
    structuredStackTrace: CallSite[],
  ) => string = MockError.prepareStackTrace;
  const result = prepareStackTrace(new Error("foo"), [
    getMockCallSite("CLI_SNAPSHOT.js", 23, 0),
  ]);
  assert(result.startsWith("Error: foo\n"));
  assert(result.includes(".ts:"), "should remap to something in 'js/'");
});

unitTest(function captureStackTrace(): void {
  function foo(): void {
    const error = new Error();
    const stack1 = error.stack!;
    Error.captureStackTrace(error, foo);
    const stack2 = error.stack!;
    // stack2 should be stack1 without the first frame.
    assertEquals(stack2, stack1.replace(/(?<=^[^\n]*\n)[^\n]*\n/, ""));
  }
  foo();
});

// FIXME(bartlomieju): no longer works after migrating
// to JavaScript runtime code
unitTest({ ignore: true }, function applySourceMap(): void {
  const result = Deno.applySourceMap({
    fileName: "CLI_SNAPSHOT.js",
    lineNumber: 23,
    columnNumber: 0,
  });
  Deno.core.print(`result: ${result}`, true);
  assert(result.fileName.endsWith(".ts"));
  assert(result.lineNumber != null);
  assert(result.columnNumber != null);
});
