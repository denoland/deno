// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

// @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
const { setPrepareStackTrace } = Deno[Deno.internal];

interface CallSite {
  getThis(): unknown;
  getTypeName(): string | null;
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
  columnNumber: number | null
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
    fileName: "CLI_SNAPSHOT.js",
    lineNumber: 23,
    columnNumber: 0,
  });
  assert(result.fileName.endsWith(".ts"));
  assert(result.lineNumber != null);
  assert(result.columnNumber != null);
});
