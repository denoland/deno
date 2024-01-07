// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

import { core, internals, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  ErrorPrototype,
  ObjectAssign,
  ObjectDefineProperties,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ObjectValues,
  Symbol,
} = primordials;
const {
  isNativeError,
} = core;
import * as event from "ext:deno_web/02_event.js";
import * as version from "ext:runtime/01_version.ts";
import * as timers from "ext:deno_web/02_timers.js";
import {
  getDefaultInspectOptions,
  getNoColor,
  inspectArgs,
  quoteString,
  setNoColorFn,
} from "ext:deno_console/01_console.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import { denoNs } from "ext:runtime/90_deno_ns.js";
import { errors } from "ext:runtime/01_errors.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import {
  unstableForWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
} from "ext:runtime/98_global_scope_shared.js";
import { memoizeLazy } from "ext:runtime/98_global_scope_window.js";
import { SymbolAsyncDispose, SymbolDispose } from "ext:deno_web/00_infra.js";

// deno-lint-ignore prefer-primordials
if (Symbol.dispose) throw "V8 supports Symbol.dispose now, no need to shim it!";
ObjectDefineProperties(Symbol, {
  dispose: {
    value: SymbolDispose,
    enumerable: false,
    writable: false,
    configurable: false,
  },
  asyncDispose: {
    value: SymbolAsyncDispose,
    enumerable: false,
    writable: false,
    configurable: false,
  },
});

let globalThis_;

function saveGlobalThisReference(gt) {
  globalThis_ = gt;
}

const opArgs = memoizeLazy(() => ops.op_bootstrap_args());
const opPid = memoizeLazy(() => ops.op_bootstrap_pid());
const opPpid = memoizeLazy(() => ops.op_ppid());
setNoColorFn(() => ops.op_bootstrap_no_color() || !ops.op_bootstrap_is_tty());

function formatException(error) {
  if (
    isNativeError(error) ||
    ObjectPrototypeIsPrototypeOf(ErrorPrototype, error)
  ) {
    return null;
  } else if (typeof error == "string") {
    return `Uncaught ${
      inspectArgs([quoteString(error, getDefaultInspectOptions())], {
        colors: !getNoColor(),
      })
    }`;
  } else {
    return `Uncaught ${inspectArgs([error], { colors: !getNoColor() })}`;
  }
}

core.registerErrorClass("NotFound", errors.NotFound);
core.registerErrorClass("PermissionDenied", errors.PermissionDenied);
core.registerErrorClass("ConnectionRefused", errors.ConnectionRefused);
core.registerErrorClass("ConnectionReset", errors.ConnectionReset);
core.registerErrorClass("ConnectionAborted", errors.ConnectionAborted);
core.registerErrorClass("NotConnected", errors.NotConnected);
core.registerErrorClass("AddrInUse", errors.AddrInUse);
core.registerErrorClass("AddrNotAvailable", errors.AddrNotAvailable);
core.registerErrorClass("BrokenPipe", errors.BrokenPipe);
core.registerErrorClass("AlreadyExists", errors.AlreadyExists);
core.registerErrorClass("InvalidData", errors.InvalidData);
core.registerErrorClass("TimedOut", errors.TimedOut);
core.registerErrorClass("Interrupted", errors.Interrupted);
core.registerErrorClass("WouldBlock", errors.WouldBlock);
core.registerErrorClass("WriteZero", errors.WriteZero);
core.registerErrorClass("UnexpectedEof", errors.UnexpectedEof);
core.registerErrorClass("BadResource", errors.BadResource);
core.registerErrorClass("Http", errors.Http);
core.registerErrorClass("Busy", errors.Busy);
core.registerErrorClass("NotSupported", errors.NotSupported);
core.registerErrorClass("FilesystemLoop", errors.FilesystemLoop);
core.registerErrorClass("IsADirectory", errors.IsADirectory);
core.registerErrorClass("NetworkUnreachable", errors.NetworkUnreachable);
core.registerErrorClass("NotADirectory", errors.NotADirectory);
core.registerErrorBuilder(
  "DOMExceptionOperationError",
  function DOMExceptionOperationError(msg) {
    return new DOMException(msg, "OperationError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionQuotaExceededError",
  function DOMExceptionQuotaExceededError(msg) {
    return new DOMException(msg, "QuotaExceededError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionNotSupportedError",
  function DOMExceptionNotSupportedError(msg) {
    return new DOMException(msg, "NotSupported");
  },
);
core.registerErrorBuilder(
  "DOMExceptionNetworkError",
  function DOMExceptionNetworkError(msg) {
    return new DOMException(msg, "NetworkError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionAbortError",
  function DOMExceptionAbortError(msg) {
    return new DOMException(msg, "AbortError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionInvalidCharacterError",
  function DOMExceptionInvalidCharacterError(msg) {
    return new DOMException(msg, "InvalidCharacterError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionDataError",
  function DOMExceptionDataError(msg) {
    return new DOMException(msg, "DataError");
  },
);

function runtimeStart(
  denoVersion,
  v8Version,
  tsVersion,
  target,
) {
  core.setMacrotaskCallback(timers.handleTimerMacrotask);
  core.setWasmStreamingCallback(fetch.handleWasmStreaming);
  core.setReportExceptionCallback(event.reportException);
  ops.op_set_format_exception_callback(formatException);
  version.setVersions(
    denoVersion,
    v8Version,
    tsVersion,
  );
  core.setBuildInfo(target);
}

core.setUnhandledPromiseRejectionHandler(processUnhandledPromiseRejection);
// Notification that the core received an unhandled promise rejection that is about to
// terminate the runtime. If we can handle it, attempt to do so.
function processUnhandledPromiseRejection(promise, reason) {
  const rejectionEvent = new event.PromiseRejectionEvent(
    "unhandledrejection",
    {
      cancelable: true,
      promise,
      reason,
    },
  );

  // Note that the handler may throw, causing a recursive "error" event
  globalThis_.dispatchEvent(rejectionEvent);

  // If event was not yet prevented, try handing it off to Node compat layer
  // (if it was initialized)
  if (
    !rejectionEvent.defaultPrevented &&
    typeof internals.nodeProcessUnhandledRejectionCallback !== "undefined"
  ) {
    internals.nodeProcessUnhandledRejectionCallback(rejectionEvent);
  }

  // If event was not prevented (or "unhandledrejection" listeners didn't
  // throw) we will let Rust side handle it.
  if (rejectionEvent.defaultPrevented) {
    return true;
  }

  return false;
}

// Delete the `console` object that V8 automaticaly adds onto the global wrapper
// object on context creation. We don't want this console object to shadow the
// `console` object exposed by the ext/node globalThis proxy.
delete globalThis.console;
// Set up global properties shared by main and worker runtime.
ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);

// Set up global properties shared by main and worker runtime that are exposed
// by unstable features if those are enabled.
function exposeUnstableFeaturesForWindowOrWorkerGlobalScope(options) {
  const { unstableFlag, unstableFeatures } = options;
  if (unstableFlag) {
    const all = ObjectValues(unstableForWindowOrWorkerGlobalScope);
    for (let i = 0; i <= all.length; i++) {
      const props = all[i];
      ObjectDefineProperties(globalThis, { ...props });
    }
  } else {
    const featureIds = ArrayPrototypeMap(
      ObjectKeys(
        unstableForWindowOrWorkerGlobalScope,
      ),
      (k) => k | 0,
    );

    for (let i = 0; i <= featureIds.length; i++) {
      const featureId = featureIds[i];
      if (ArrayPrototypeIncludes(unstableFeatures, featureId)) {
        const props = unstableForWindowOrWorkerGlobalScope[featureId];
        ObjectDefineProperties(globalThis, { ...props });
      }
    }
  }
}

// FIXME(bartlomieju): temporarily add whole `Deno.core` to
// `Deno[Deno.internal]` namespace. It should be removed and only necessary
// methods should be left there.
ObjectAssign(internals, { core });
const internalSymbol = Symbol("Deno.internal");
const finalDenoNs = {
  internal: internalSymbol,
  [internalSymbol]: internals,
  resources: core.resources,
  close: core.close,
  ...denoNs,
  // Deno.test and Deno.bench are noops here, but kept for compatibility; so
  // that they don't cause errors when used outside of `deno test`/`deno bench`
  // contexts.
  test: () => {},
  bench: () => {},
};

const {
  denoVersion,
  tsVersion,
  v8Version,
  target,
} = ops.op_snapshot_options();

export {
  denoVersion,
  exposeUnstableFeaturesForWindowOrWorkerGlobalScope,
  finalDenoNs,
  opArgs,
  opPid,
  opPpid,
  runtimeStart,
  saveGlobalThisReference,
  target,
  tsVersion,
  v8Version,
};
