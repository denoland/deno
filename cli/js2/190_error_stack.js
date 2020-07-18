// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  // Some of the code here is adapted directly from V8 and licensed under a BSD
  // style license available here: https://github.com/v8/v8/blob/24886f2d1c565287d33d71e4109a53bf0b54b75c/LICENSE.v8
  const colors = window.__bootstrap.colors;
  const assert = window.__bootstrap.util.assert;
  const internals = window.__bootstrap.internals;
  const dispatchJson = window.__bootstrap.dispatchJson;

  function opFormatDiagnostics(items) {
    return dispatchJson.sendSync("op_format_diagnostic", { items });
  }

  function opApplySourceMap(location) {
    const res = dispatchJson.sendSync("op_apply_source_map", location);
    return {
      fileName: res.fileName,
      lineNumber: res.lineNumber,
      columnNumber: res.columnNumber,
    };
  }

  function patchCallSite(callSite, location) {
    return {
      getThis() {
        return callSite.getThis();
      },
      getTypeName() {
        return callSite.getTypeName();
      },
      getFunction() {
        return callSite.getFunction();
      },
      getFunctionName() {
        return callSite.getFunctionName();
      },
      getMethodName() {
        return callSite.getMethodName();
      },
      getFileName() {
        return location.fileName;
      },
      getLineNumber() {
        return location.lineNumber;
      },
      getColumnNumber() {
        return location.columnNumber;
      },
      getEvalOrigin() {
        return callSite.getEvalOrigin();
      },
      isToplevel() {
        return callSite.isToplevel();
      },
      isEval() {
        return callSite.isEval();
      },
      isNative() {
        return callSite.isNative();
      },
      isConstructor() {
        return callSite.isConstructor();
      },
      isAsync() {
        return callSite.isAsync();
      },
      isPromiseAll() {
        return callSite.isPromiseAll();
      },
      getPromiseIndex() {
        return callSite.getPromiseIndex();
      },
    };
  }

  function getMethodCall(callSite) {
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

  function getFileLocation(callSite, internal = false) {
    const cyan = internal ? colors.gray : colors.cyan;
    const yellow = internal ? colors.gray : colors.yellow;
    const black = internal ? colors.gray : (s) => s;
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

  function callSiteToString(callSite, internal = false) {
    const cyan = internal ? colors.gray : colors.cyan;
    const black = internal ? colors.gray : (s) => s;

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
        colors.italic(
          black(`Promise.all (index ${callSite.getPromiseIndex()})`),
        ),
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
      result += getFileLocation(callSite, internal);
      return result;
    }

    result += ` ${black("(")}${getFileLocation(callSite, internal)}${
      black(")")
    }`;
    return result;
  }

  function evaluateCallSite(callSite) {
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
    error,
    callSites,
  ) {
    const mappedCallSites = callSites.map(
      (callSite) => {
        const fileName = callSite.getFileName();
        const lineNumber = callSite.getLineNumber();
        const columnNumber = callSite.getColumnNumber();
        if (fileName && lineNumber != null && columnNumber != null) {
          return patchCallSite(
            callSite,
            opApplySourceMap({
              fileName,
              lineNumber,
              columnNumber,
            }),
          );
        }
        return callSite;
      },
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
        .map((s) => `    at ${colors.stripColor(s)}`)
        .join("\n")
    );
  }

  function setPrepareStackTrace(ErrorConstructor) {
    ErrorConstructor.prepareStackTrace = prepareStackTrace;
  }

  internals.exposeForTest("setPrepareStackTrace", setPrepareStackTrace);

  window.__bootstrap.errorStack = {
    setPrepareStackTrace,
    opApplySourceMap,
    opFormatDiagnostics,
  };
})(this);
