System.register(
  "$deno$/error_stack.ts",
  [
    "$deno$/colors.ts",
    "$deno$/ops/errors.ts",
    "$deno$/util.ts",
    "$deno$/internals.ts",
  ],
  function (exports_16, context_16) {
    "use strict";
    let colors, errors_ts_2, util_ts_2, internals_ts_1;
    const __moduleName = context_16 && context_16.id;
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
    function getFileLocation(callSite, isInternal = false) {
      const cyan = isInternal ? colors.gray : colors.cyan;
      const yellow = isInternal ? colors.gray : colors.yellow;
      const black = isInternal ? colors.gray : (s) => s;
      if (callSite.isNative()) {
        return cyan("native");
      }
      let result = "";
      const fileName = callSite.getFileName();
      if (!fileName && callSite.isEval()) {
        const evalOrigin = callSite.getEvalOrigin();
        util_ts_2.assert(evalOrigin != null);
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
    function callSiteToString(callSite, isInternal = false) {
      const cyan = isInternal ? colors.gray : colors.cyan;
      const black = isInternal ? colors.gray : (s) => s;
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
            black(`Promise.all (index ${callSite.getPromiseIndex()})`)
          )
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
    function prepareStackTrace(error, structuredStackTrace) {
      Object.defineProperties(error, {
        __callSiteEvals: { value: [] },
        __formattedFrames: { value: [] },
      });
      const errorString =
        `${error.name}: ${error.message}\n` +
        structuredStackTrace
          .map((callSite) => {
            const fileName = callSite.getFileName();
            const lineNumber = callSite.getLineNumber();
            const columnNumber = callSite.getColumnNumber();
            if (fileName && lineNumber != null && columnNumber != null) {
              return patchCallSite(
                callSite,
                errors_ts_2.applySourceMap({
                  fileName,
                  lineNumber,
                  columnNumber,
                })
              );
            }
            return callSite;
          })
          .map((callSite) => {
            // @ts-ignore
            error.__callSiteEvals.push(
              Object.freeze(evaluateCallSite(callSite))
            );
            const isInternal =
              callSite.getFileName()?.startsWith("$deno$") ?? false;
            const string = callSiteToString(callSite, isInternal);
            // @ts-ignore
            error.__formattedFrames.push(string);
            return `    at ${colors.stripColor(string)}`;
          })
          .join("\n");
      // @ts-ignore
      Object.freeze(error.__callSiteEvals);
      // @ts-ignore
      Object.freeze(error.__formattedFrames);
      return errorString;
    }
    // @internal
    function setPrepareStackTrace(ErrorConstructor) {
      ErrorConstructor.prepareStackTrace = prepareStackTrace;
    }
    exports_16("setPrepareStackTrace", setPrepareStackTrace);
    return {
      setters: [
        function (colors_1) {
          colors = colors_1;
        },
        function (errors_ts_2_1) {
          errors_ts_2 = errors_ts_2_1;
        },
        function (util_ts_2_1) {
          util_ts_2 = util_ts_2_1;
        },
        function (internals_ts_1_1) {
          internals_ts_1 = internals_ts_1_1;
        },
      ],
      execute: function () {
        internals_ts_1.exposeForTest(
          "setPrepareStackTrace",
          setPrepareStackTrace
        );
      },
    };
  }
);
