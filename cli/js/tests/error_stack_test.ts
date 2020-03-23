// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

// @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { setPrepareStackTrace } = Deno[Deno.symbols.internal];

interface CallSite {
  getThis(): unknown;
  getTypeName(): string;
  getFunction(): Function;
  getFunctionName(): string;
  getMethodName(): string;
  getFileName(): string;
  getLineNumber(): number | null;
  getColumnNumber(): number | null;
  getEvalOrigin(): string | null;
  isToplevel(): boolean;
  isEval(): boolean;
  isNative(): boolean;
  isConstructor(): boolean;
  isAsync(): boolean;
  isPromiseAll(): boolean;
  getPromiseIndex(): number | null;
}

function getMockCallSite(
  filename: string,
  line: number | null,
  column: number | null
): CallSite {
  return {
    getThis(): unknown {
      return undefined;
    },
    getTypeName(): string {
      return "";
    },
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
      return filename;
    },
    getLineNumber(): number | null {
      return line;
    },
    getColumnNumber(): number | null {
      return column;
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

unitTest(function prepareStackTrace(): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const MockError = {} as any;
  setPrepareStackTrace(MockError);
  assert(typeof MockError.prepareStackTrace === "function");
  const prepareStackTrace: (
    error: Error,
    structuredStackTrace: CallSite[]
  ) => string = MockError.prepareStackTrace;
  const result = prepareStackTrace(new Error("foo"), [
    getMockCallSite("CLI_SNAPSHOT.js", 23, 0),
  ]);
  assert(result.startsWith("Error: foo\n"));
  assert(result.includes(".ts:"), "should remap to something in 'js/'");
});

unitTest(function applySourceMap(): void {
  const result = Deno.applySourceMap({
    filename: "CLI_SNAPSHOT.js",
    line: 23,
    column: 0,
  });
  assert(result.filename.endsWith(".ts"));
  assert(result.line != null);
  assert(result.column != null);
});
