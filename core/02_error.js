// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = Deno.core;
  const {
    Error,
    ObjectFreeze,
    ObjectAssign,
    StringPrototypeStartsWith,
    StringPrototypeEndsWith,
    ObjectDefineProperties,
    ArrayPrototypePush,
    ArrayPrototypeMap,
    ArrayPrototypeJoin,
  } = window.__bootstrap.primordials;

  // Some of the code here is adapted directly from V8 and licensed under a BSD
  // style license available here: https://github.com/v8/v8/blob/24886f2d1c565287d33d71e4109a53bf0b54b75c/LICENSE.v8
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

  // Keep in sync with `cli/fmt_errors.rs`.
  function formatLocation(callSite) {
    if (callSite.isNative()) {
      return "native";
    }

    let result = "";

    const fileName = callSite.getFileName();

    if (fileName) {
      result += core.opSync("op_format_file_name", fileName);
    } else {
      if (callSite.isEval()) {
        const evalOrigin = callSite.getEvalOrigin();
        if (evalOrigin == null) {
          throw new Error("assert evalOrigin");
        }
        result += `${evalOrigin}, `;
      }
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

  // Keep in sync with `cli/fmt_errors.rs`.
  function formatCallSite(callSite) {
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
      const typeName = callSite.getTypeName();
      const methodName = callSite.getMethodName();

      if (functionName) {
        if (typeName) {
          if (!StringPrototypeStartsWith(functionName, typeName)) {
            result += `${typeName}.`;
          }
        }
        result += functionName;
        if (methodName) {
          if (!StringPrototypeEndsWith(functionName, methodName)) {
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
      result += formatLocation(callSite);
      return result;
    }

    result += ` (${formatLocation(callSite)})`;
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

  /** A function that can be used as `Error.prepareStackTrace`. */
  function prepareStackTrace(
    error,
    callSites,
  ) {
    const mappedCallSites = ArrayPrototypeMap(callSites, (callSite) => {
      const fileName = callSite.getFileName();
      const lineNumber = callSite.getLineNumber();
      const columnNumber = callSite.getColumnNumber();
      if (fileName && lineNumber != null && columnNumber != null) {
        return patchCallSite(
          callSite,
          core.opSync("op_apply_source_map", {
            fileName,
            lineNumber,
            columnNumber,
          }),
        );
      }
      return callSite;
    });
    ObjectDefineProperties(error, {
      __callSiteEvals: { value: [], configurable: true },
    });
    const formattedCallSites = [];
    for (const callSite of mappedCallSites) {
      ArrayPrototypePush(error.__callSiteEvals, evaluateCallSite(callSite));
      ArrayPrototypePush(
        formattedCallSites,
        formatCallSite(callSite),
      );
    }
    const message = error.message !== undefined ? error.message : "";
    const name = error.name !== undefined ? error.name : "Error";
    let messageLine;
    if (name != "" && message != "") {
      messageLine = `${name}: ${message}`;
    } else if ((name || message) != "") {
      messageLine = name || message;
    } else {
      messageLine = "";
    }
    return messageLine +
      ArrayPrototypeJoin(
        ArrayPrototypeMap(formattedCallSites, (s) => `\n    at ${s}`),
        "",
      );
  }

  ObjectAssign(globalThis.__bootstrap.core, { prepareStackTrace });
  ObjectFreeze(globalThis.__bootstrap.core);
})(this);
