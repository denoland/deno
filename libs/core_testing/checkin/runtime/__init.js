// Copyright 2018-2025 the Deno authors. MIT license.
import * as async from "checkin:async";
import * as console from "checkin:console";
import * as error from "checkin:error";
import * as timers from "checkin:timers";
import * as worker from "checkin:worker";
import * as throw_ from "checkin:throw";
import * as object from "checkin:object";
import * as callsite from "checkin:callsite";
async;
error;
throw_;
object;
callsite;

globalThis.console = console.console;
globalThis.setTimeout = timers.setTimeout;
globalThis.setInterval = timers.setInterval;
globalThis.clearTimeout = timers.clearTimeout;
globalThis.clearInterval = timers.clearInterval;
globalThis.Worker = worker.Worker;
Deno.core.addMainModuleHandler((module) => {
  if (onMainModuleCb) onMainModuleCb(module);
});
let onMainModuleCb = () => {};
Reflect.defineProperty(globalThis, "onmainmodule", {
  set: (cb) => {
    onMainModuleCb = cb;
  },
});
Reflect.defineProperty(globalThis, "onerror", {
  set: (cb) => {
    if (cb) {
      Deno.core.setReportExceptionCallback((error) => {
        let defaultPrevented = false;
        cb({
          error,
          preventDefault: () => defaultPrevented = true,
        });
        if (!defaultPrevented) {
          Deno.core.reportUnhandledException(error);
        }
      });
    } else {
      Deno.core.setReportExceptionCallback(null);
    }
  },
});
Reflect.defineProperty(globalThis, "onunhandledrejection", {
  set: (cb) => {
    if (cb) {
      Deno.core.setUnhandledPromiseRejectionHandler((promise, reason) => {
        let defaultPrevented = false;
        cb({
          promise,
          reason,
          preventDefault: () => defaultPrevented = true,
        });
        return defaultPrevented;
      });
    } else {
      Deno.core.setUnhandledPromiseRejectionHandler(null);
    }
  },
});
Reflect.defineProperty(globalThis, "onrejectionhandled", {
  set: (cb) => {
    if (cb) {
      Deno.core.setHandledPromiseRejectionHandler((promise, reason) => {
        cb({
          promise,
          reason,
        });
      });
    } else {
      Deno.core.setHandledPromiseRejectionHandler(null);
    }
  },
});
Deno.unrefTimer = timers.unrefTimer;
Deno.refTimer = timers.refTimer;
