// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any
type Thing = {
  name: string;
};

try {
  throw new Error("This is an error");
} catch (e) {
  (Error as any).prepareStackTrace = (_: any, stack: any) => {
    return stack.map((s: any) => ({
      filename: s.getFileName(),
      methodName: s.getMethodName(),
      functionName: s.getFunctionName(),
      lineNumber: s.getLineNumber(),
      columnNumber: s.getColumnNumber(),
    }));
  };
  console.log((e as Error).stack);
}
