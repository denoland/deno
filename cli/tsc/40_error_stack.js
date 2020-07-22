// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  // Some of the code here is adapted directly from V8 and licensed under a BSD
  // style license available here: https://github.com/v8/v8/blob/24886f2d1c565287d33d71e4109a53bf0b54b75c/LICENSE.v8
  const assert = window.__bootstrap.util.assert;

  // https://github.com/chalk/ansi-regex/blob/2b56fb0c7a07108e5b54241e8faec160d393aedb/index.js
  const ANSI_PATTERN = new RegExp(
    [
      "[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:[a-zA-Z\\d]*(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)",
      "(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))",
    ].join("|"),
    "g",
  );

  function stripColor(string) {
    return string.replace(ANSI_PATTERN, "");
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

  function getFileLocation(callSite) {
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
      result += `:${lineNumber.toString()}`;

      const columnNumber = callSite.getColumnNumber();
      if (columnNumber != null) {
        result += `:${columnNumber.toString()}`;
      }
    }

    return result;
  }

  function callSiteToString(callSite) {
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
      result += `Promise.all (index ${callSite.getPromiseIndex()})`;
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
            {
              fileName,
              lineNumber,
              columnNumber,
            },
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
      error.__formattedFrames.push(callSiteToString(callSite));
    }
    Object.freeze(error.__callSiteEvals);
    Object.freeze(error.__formattedFrames);
    return (
      `${error.name}: ${error.message}\n` +
      error.__formattedFrames
        .map((s) => `    at ${stripColor(s)}`)
        .join("\n")
    );
  }

  function setPrepareStackTrace(ErrorConstructor) {
    ErrorConstructor.prepareStackTrace = prepareStackTrace;
  }

  window.__bootstrap.errorStack = {
    setPrepareStackTrace,
  };
})(this);
