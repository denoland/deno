// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some of the code here is adapted directly from V8 and licensed under a BSD
// style license available here: https://github.com/v8/v8/blob/24886f2d1c565287d33d71e4109a53bf0b54b75c/LICENSE.v8
import { applySourceMap, Location } from "./ops/errors.ts";
import { assert } from "./util.ts";
import { exposeForTest } from "./internals.ts";

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
    },
  };
}

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
                column,
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

// @internal
export function setPrepareStackTrace(ErrorConstructor: typeof Error): void {
  ErrorConstructor.prepareStackTrace = prepareStackTrace;
}

exposeForTest("setPrepareStackTrace", setPrepareStackTrace);
