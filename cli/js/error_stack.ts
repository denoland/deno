// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some of the code here is adapted directly from V8 and licensed under a BSD
// style license available here: https://github.com/v8/v8/blob/24886f2d1c565287d33d71e4109a53bf0b54b75c/LICENSE.v8
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";
import { assert } from "./util.ts";
import { exposeForTest } from "./internals.ts";

export interface Location {
  /** The full url for the module, e.g. `file://some/file.ts` or
   * `https://some/file.ts`. */
  filename: string;

  /** The line number in the file.  It is assumed to be 1-indexed. */
  line: number;

  /** The column number in the file.  It is assumed to be 1-indexed. */
  column: number;
}

/** Given a current location in a module, lookup the source location and
 * return it.
 *
 * When Deno transpiles code, it keep source maps of the transpiled code.  This
 * function can be used to lookup the original location.  This is automatically
 * done when accessing the `.stack` of an error, or when an uncaught error is
 * logged.  This function can be used to perform the lookup for creating better
 * error handling.
 *
 * **Note:** `line` and `column` are 1 indexed, which matches display
 * expectations, but is not typical of most index numbers in Deno.
 *
 * An example:
 *
 *       const orig = Deno.applySourceMap({
 *         location: "file://my/module.ts",
 *         line: 5,
 *         column: 15
 *       });
 *       console.log(`${orig.filename}:${orig.line}:${orig.column}`);
 *
 */
export function applySourceMap(location: Location): Location {
  const { filename, line, column } = location;
  // On this side, line/column are 1 based, but in the source maps, they are
  // 0 based, so we have to convert back and forth
  const res = sendSync(dispatch.OP_APPLY_SOURCE_MAP, {
    filename,
    line: line - 1,
    column: column - 1
  });
  return {
    filename: res.filename,
    line: res.line + 1,
    column: res.column + 1
  };
}

/** Mutate the call site so that it returns the location, instead of its
 * original location.
 */
function patchCallSite(callSite: CallSite, location: Location): CallSite {
  return {
    getThis(): unknown {
      return callSite.getThis();
    },
    getTypeName(): string {
      return callSite.getTypeName();
    },
    getFunction(): Function {
      return callSite.getFunction();
    },
    getFunctionName(): string {
      return callSite.getFunctionName();
    },
    getMethodName(): string {
      return callSite.getMethodName();
    },
    getFileName(): string {
      return location.filename;
    },
    getLineNumber(): number {
      return location.line;
    },
    getColumnNumber(): number {
      return location.column;
    },
    getEvalOrigin(): string | null {
      return callSite.getEvalOrigin();
    },
    isToplevel(): boolean {
      return callSite.isToplevel();
    },
    isEval(): boolean {
      return callSite.isEval();
    },
    isNative(): boolean {
      return callSite.isNative();
    },
    isConstructor(): boolean {
      return callSite.isConstructor();
    },
    isAsync(): boolean {
      return callSite.isAsync();
    },
    isPromiseAll(): boolean {
      return callSite.isPromiseAll();
    },
    getPromiseIndex(): number | null {
      return callSite.getPromiseIndex();
    }
  };
}

/** Return a string representations of a CallSite's method call name
 *
 * This is adapted directly from V8.
 */
function getMethodCall(callSite: CallSite): string {
  let result = "";

  const typeName = callSite.getTypeName();
  const methodName = callSite.getMethodName();
  const functionName = callSite.getFunctionName();

  if (functionName) {
    if (typeName) {
      const startsWithTypeName = functionName.startsWith(typeName);
      if (!startsWithTypeName) {
        result += `${typeName}.`;
      }
    }
    result += functionName;

    if (methodName) {
      if (!functionName.endsWith(methodName)) {
        result += ` [as ${methodName}]`;
      }
    }
  } else {
    if (typeName) {
      result += `${typeName}.`;
    }
    if (methodName) {
      result += methodName;
    } else {
      result += "<anonymous>";
    }
  }

  return result;
}

/** Return a string representations of a CallSite's file location
 *
 * This is adapted directly from V8.
 */
function getFileLocation(callSite: CallSite): string {
  if (callSite.isNative()) {
    return "native";
  }

  let result = "";

  const fileName = callSite.getFileName();
  if (!fileName && callSite.isEval()) {
    const evalOrigin = callSite.getEvalOrigin();
    assert(evalOrigin != null);
    result += `${evalOrigin}, `;
  }

  if (fileName) {
    result += fileName;
  } else {
    result += "<anonymous>";
  }

  const lineNumber = callSite.getLineNumber();
  if (lineNumber != null) {
    result += `:${lineNumber}`;

    const columnNumber = callSite.getColumnNumber();
    if (columnNumber != null) {
      result += `:${columnNumber}`;
    }
  }

  return result;
}

/** Convert a CallSite to a string.
 *
 * This is adapted directly from V8.
 */
function callSiteToString(callSite: CallSite): string {
  let result = "";
  const functionName = callSite.getFunctionName();

  const isTopLevel = callSite.isToplevel();
  const isAsync = callSite.isAsync();
  const isPromiseAll = callSite.isPromiseAll();
  const isConstructor = callSite.isConstructor();
  const isMethodCall = !(isTopLevel || isConstructor);

  if (isAsync) {
    result += "async ";
  }
  if (isPromiseAll) {
    result += `Promise.all (index ${callSite.getPromiseIndex})`;
    return result;
  }
  if (isMethodCall) {
    result += getMethodCall(callSite);
  } else if (isConstructor) {
    result += "new ";
    if (functionName) {
      result += functionName;
    } else {
      result += "<anonymous>";
    }
  } else if (functionName) {
    result += functionName;
  } else {
    result += getFileLocation(callSite);
    return result;
  }

  result += ` (${getFileLocation(callSite)})`;
  return result;
}

/** A replacement for the default stack trace preparer which will op into Rust
 * to apply source maps to individual sites
 */
function prepareStackTrace(
  error: Error,
  structuredStackTrace: CallSite[]
): string {
  return (
    `${error.name}: ${error.message}\n` +
    structuredStackTrace
      .map(
        (callSite): CallSite => {
          const filename = callSite.getFileName();
          const line = callSite.getLineNumber();
          const column = callSite.getColumnNumber();
          if (filename && line != null && column != null) {
            return patchCallSite(
              callSite,
              applySourceMap({
                filename,
                line,
                column
              })
            );
          }
          return callSite;
        }
      )
      .map((callSite): string => `    at ${callSiteToString(callSite)}`)
      .join("\n")
  );
}

/** Sets the `prepareStackTrace` method on the Error constructor which will
 * op into Rust to remap source code for caught errors where the `.stack` is
 * being accessed.
 *
 * See: https://v8.dev/docs/stack-trace-api
 */
// @internal
export function setPrepareStackTrace(ErrorConstructor: typeof Error): void {
  ErrorConstructor.prepareStackTrace = prepareStackTrace;
}

exposeForTest("setPrepareStackTrace", setPrepareStackTrace);
