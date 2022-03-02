// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { ErrorEvent } = window;
  const { Error, TypeError } = window.__bootstrap.primordials;
  const webidl = window.__bootstrap.webidl;

  let errorReported = false;

  function handleReportErrorMacrotask() {
    if (errorReported) {
      errorReported = false;
      throw new Error("Unhandled error event from 'reportError()'.");
    }
    return true;
  }

  let printException = undefined;

  function setPrintException(fn) {
    printException = fn;
  }

  function checkThis(thisArg) {
    if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
      throw new TypeError("Illegal invocation");
    }
  }

  function reportError(error) {
    checkThis(this);
    const prefix = "Failed to execute 'reportError' on 'Window'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    const jsError = core.destructureError(error);
    const message = jsError.message;
    let filename = "";
    let lineno = 0;
    let colno = 0;
    if (jsError.frames.length > 0) {
      filename = jsError.frames[0].fileName;
      lineno = jsError.frames[0].lineNumber;
      colno = jsError.frames[0].columnNumber;
    } else {
      const jsError = core.destructureError(new Error());
      filename = jsError.frames[1].fileName;
      lineno = jsError.frames[1].lineNumber;
      colno = jsError.frames[1].columnNumber;
    }
    const event = new ErrorEvent("error", {
      cancelable: true,
      message,
      filename,
      lineno,
      colno,
      error,
    });
    if (window.dispatchEvent(event)) {
      printException?.(jsError);
      // TODO(nayeemrmn): We need to throw an uncatchable error here that leads
      // to termination of the current worker. We do this by scheduling one to
      // be thrown in a new macrotask using this `errorReported` flag. Consider
      // a new `JsRuntime` binding which immediately fails on an uncatchable
      // error. May not be worth it.
      errorReported = true;
    }
  }

  window.__bootstrap.reportError = {
    handleReportErrorMacrotask,
    reportError,
    setPrintException,
  };
})(this);
