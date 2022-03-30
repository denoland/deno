// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { ErrorEvent } = window;
  const { Error, StringPrototypeStartsWith, TypeError } =
    window.__bootstrap.primordials;
  const webidl = window.__bootstrap.webidl;

  let errorReported = false;

  function handleReportExceptionMacrotask() {
    if (errorReported) {
      errorReported = false;
      throw new Error(`Unhandled error event.`);
    }
    return true;
  }

  let printException = undefined;

  function setPrintException(fn) {
    printException = fn;
  }

  let reportExceptionStackedCalls = 0;

  // https://html.spec.whatwg.org/#report-the-exception
  function reportException(error) {
    reportExceptionStackedCalls++;
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
      for (const frame of jsError.frames) {
        if (
          typeof frame.fileName == "string" &&
          !StringPrototypeStartsWith(frame.fileName, "deno:")
        ) {
          filename = frame.fileName;
          lineno = frame.lineNumber;
          colno = frame.columnNumber;
          break;
        }
      }
    }
    const event = new ErrorEvent("error", {
      cancelable: true,
      message,
      filename,
      lineno,
      colno,
      error,
    });
    // Avoid recursing `reportException()` via error handlers more than once.
    if (reportExceptionStackedCalls > 1 || window.dispatchEvent(event)) {
      printException?.(jsError);
      // TODO(nayeemrmn): We need to throw an uncatchable error here that leads
      // to termination of the runtime. We do this by scheduling one to be
      // thrown in a new macrotask using this `errorReported` flag. Consider
      // a new `JsRuntime` binding which immediately fails on an uncatchable
      // error. May not be worth it.
      errorReported = true;
    }
    reportExceptionStackedCalls--;
  }

  function checkThis(thisArg) {
    if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
      throw new TypeError("Illegal invocation");
    }
  }

  // https://html.spec.whatwg.org/#dom-reporterror
  function reportError(error) {
    checkThis(this);
    const prefix = "Failed to call 'reportError'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    reportException(error);
  }

  window.__bootstrap.reportError = {
    handleReportExceptionMacrotask,
    reportError,
    setPrintException,
  };
})(this);
