// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some of the code here is adapted directly from V8 and licensed under a BSD
// style license available here: https://github.com/v8/v8/blob/24886f2d1c565287d33d71e4109a53bf0b54b75c/LICENSE.v8
import * as colors from "./colors.ts";
import { applySourceMap, Location } from "./ops/errors.ts";
import { assert } from "./util.ts";
import { exposeForTest } from "./internals.ts";

function patchCallSite(callSite: CallSite, location: Location): CallSite {
  return {
    getThis(): unknown {
      return callSite.getThis();
    },
    getTypeName(): string | null {
      return callSite.getTypeName();
    },
    getFunction(): Function | null {
      return callSite.getFunction();
    },
    getFunctionName(): string | null {
      return callSite.getFunctionName();
    },
    getMethodName(): string | null {
      return callSite.getMethodName();
    },
    getFileName(): string | null {
      return location.fileName;
    },
    getLineNumber(): number {
      return location.lineNumber;
    },
    getColumnNumber(): number {
      return location.columnNumber;
    },
    getEvalOrigin(): string | null {
      return callSite.getEvalOrigin();
    },
    isToplevel(): boolean | null {
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

function getFileLocation(callSite: CallSite, isInternal = false): string {
  const cyan = isInternal ? colors.gray : colors.cyan;
  const yellow = isInternal ? colors.gray : colors.yellow;
  const black = isInternal ? colors.gray : (s: string): string => s;
  if (callSite.isNative()) {
    return cyan("native");
  }

  let result = "";

  const fileName = callSite.getFileName();
  if (!fileName && callSite.isEval()) {
    const evalOrigin = callSite.getEvalOrigin();
    assert(evalOrigin != null);
    result += cyan(`${evalOrigin}, `);
  }

  if (fileName) {
    result += cyan(fileName);
  } else {
    result += cyan("<anonymous>");
  }

  const lineNumber = callSite.getLineNumber();
  if (lineNumber != null) {
    result += `${black(":")}${yellow(lineNumber.toString())}`;

    const columnNumber = callSite.getColumnNumber();
    if (columnNumber != null) {
      result += `${black(":")}${yellow(columnNumber.toString())}`;
    }
  }

  return result;
}

function callSiteToString(callSite: CallSite, isInternal = false): string {
  const cyan = isInternal ? colors.gray : colors.cyan;
  const black = isInternal ? colors.gray : (s: string): string => s;

  let result = "";
  const functionName = callSite.getFunctionName();

  const isTopLevel = callSite.isToplevel();
  const isAsync = callSite.isAsync();
  const isPromiseAll = callSite.isPromiseAll();
  const isConstructor = callSite.isConstructor();
  const isMethodCall = !(isTopLevel || isConstructor);

  if (isAsync) {
    result += colors.gray("async ");
  }
  if (isPromiseAll) {
    result += colors.bold(
      colors.italic(black(`Promise.all (index ${callSite.getPromiseIndex()})`))
    );
    return result;
  }
  if (isMethodCall) {
    result += colors.bold(colors.italic(black(getMethodCall(callSite))));
  } else if (isConstructor) {
    result += colors.gray("new ");
    if (functionName) {
      result += colors.bold(colors.italic(black(functionName)));
    } else {
      result += cyan("<anonymous>");
    }
  } else if (functionName) {
    result += colors.bold(colors.italic(black(functionName)));
  } else {
    result += getFileLocation(callSite, isInternal);
    return result;
  }

  result += ` ${black("(")}${getFileLocation(callSite, isInternal)}${black(
    ")"
  )}`;
  return result;
}

interface CallSiteEval {
  this: unknown;
  typeName: string | null;
  function: Function | null;
  functionName: string | null;
  methodName: string | null;
  fileName: string | null;
  lineNumber: number | null;
  columnNumber: number | null;
  evalOrigin: string | null;
  isToplevel: boolean | null;
  isEval: boolean;
  isNative: boolean;
  isConstructor: boolean;
  isAsync: boolean;
  isPromiseAll: boolean;
  promiseIndex: number | null;
}

function evaluateCallSite(callSite: CallSite): CallSiteEval {
  return {
    this: callSite.getThis(),
    typeName: callSite.getTypeName(),
    function: callSite.getFunction(),
    functionName: callSite.getFunctionName(),
    methodName: callSite.getMethodName(),
    fileName: callSite.getFileName(),
    lineNumber: callSite.getLineNumber(),
    columnNumber: callSite.getColumnNumber(),
    evalOrigin: callSite.getEvalOrigin(),
    isToplevel: callSite.isToplevel(),
    isEval: callSite.isEval(),
    isNative: callSite.isNative(),
    isConstructor: callSite.isConstructor(),
    isAsync: callSite.isAsync(),
    isPromiseAll: callSite.isPromiseAll(),
    promiseIndex: callSite.getPromiseIndex(),
  };
}

function prepareStackTrace(
  error: Error & {
    __callSiteEvals: CallSiteEval[];
    __formattedFrames: string[];
  },
  callSites: CallSite[]
): string {
  const mappedCallSites = callSites.map(
    (callSite): CallSite => {
      const fileName = callSite.getFileName();
      const lineNumber = callSite.getLineNumber();
      const columnNumber = callSite.getColumnNumber();
      if (fileName && lineNumber != null && columnNumber != null) {
        return patchCallSite(
          callSite,
          applySourceMap({
            fileName,
            lineNumber,
            columnNumber,
          })
        );
      }
      return callSite;
    }
  );
  Object.defineProperties(error, {
    __callSiteEvals: { value: [], configurable: true },
    __formattedFrames: { value: [], configurable: true },
  });
  for (const callSite of mappedCallSites) {
    error.__callSiteEvals.push(Object.freeze(evaluateCallSite(callSite)));
    const isInternal = callSite.getFileName()?.startsWith("$deno$") ?? false;
    error.__formattedFrames.push(callSiteToString(callSite, isInternal));
  }
  Object.freeze(error.__callSiteEvals);
  Object.freeze(error.__formattedFrames);
  return (
    `${error.name}: ${error.message}\n` +
    error.__formattedFrames
      .map((s: string) => `    at ${colors.stripColor(s)}`)
      .join("\n")
  );
}

// @internal
export function setPrepareStackTrace(ErrorConstructor: typeof Error): void {
  ErrorConstructor.prepareStackTrace = prepareStackTrace;
}

exposeForTest("setPrepareStackTrace", setPrepareStackTrace);
