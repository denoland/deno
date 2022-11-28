// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = Deno.core;
  const ops = core.ops;
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

  // Keep in sync with `cli/fmt_errors.rs`.
  function formatLocation(cse) {
    if (cse.isNative) {
      return "native";
    }
    let result = "";
    if (cse.fileName) {
      result += ops.op_format_file_name(cse.fileName);
    } else {
      if (cse.isEval) {
        if (cse.evalOrigin == null) {
          throw new Error("assert evalOrigin");
        }
        result += `${cse.evalOrigin}, `;
      }
      result += "<anonymous>";
    }
    if (cse.lineNumber != null) {
      result += `:${cse.lineNumber}`;
      if (cse.columnNumber != null) {
        result += `:${cse.columnNumber}`;
      }
    }
    return result;
  }

  // Keep in sync with `cli/fmt_errors.rs`.
  function formatCallSiteEval(cse) {
    let result = "";
    if (cse.isAsync) {
      result += "async ";
    }
    if (cse.isPromiseAll) {
      result += `Promise.all (index ${cse.promiseIndex})`;
      return result;
    }
    const isMethodCall = !(cse.isToplevel || cse.isConstructor);
    if (isMethodCall) {
      if (cse.functionName) {
        if (cse.typeName) {
          if (!StringPrototypeStartsWith(cse.functionName, cse.typeName)) {
            result += `${cse.typeName}.`;
          }
        }
        result += cse.functionName;
        if (cse.methodName) {
          if (!StringPrototypeEndsWith(cse.functionName, cse.methodName)) {
            result += ` [as ${cse.methodName}]`;
          }
        }
      } else {
        if (cse.typeName) {
          result += `${cse.typeName}.`;
        }
        if (cse.methodName) {
          result += cse.methodName;
        } else {
          result += "<anonymous>";
        }
      }
    } else if (cse.isConstructor) {
      result += "new ";
      if (cse.functionName) {
        result += cse.functionName;
      } else {
        result += "<anonymous>";
      }
    } else if (cse.functionName) {
      result += cse.functionName;
    } else {
      result += formatLocation(cse);
      return result;
    }

    result += ` (${formatLocation(cse)})`;
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

  function sourceMapCallSiteEval(cse) {
    if (cse.fileName && cse.lineNumber != null && cse.columnNumber != null) {
      return { ...cse, ...ops.op_apply_source_map(cse) };
    }
    return cse;
  }

  /** A function that can be used as `Error.prepareStackTrace`. */
  function prepareStackTrace(error, callSites) {
    let callSiteEvals = ArrayPrototypeMap(callSites, evaluateCallSite);
    callSiteEvals = ArrayPrototypeMap(callSiteEvals, sourceMapCallSiteEval);
    ObjectDefineProperties(error, {
      __callSiteEvals: { __proto__: null, value: [], configurable: true },
    });
    const formattedCallSites = [];
    for (const cse of callSiteEvals) {
      ArrayPrototypePush(error.__callSiteEvals, cse);
      ArrayPrototypePush(formattedCallSites, formatCallSiteEval(cse));
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
