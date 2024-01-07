// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayPrototypeIncludes,
  DateNow,
  Error,
  FunctionPrototypeCall,
  ObjectAssign,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectSetPrototypeOf,
  PromisePrototypeThen,
  PromiseResolve,
} = primordials;
import * as util from "ext:runtime/06_util.js";
import * as event from "ext:deno_web/02_event.js";
import * as location from "ext:deno_web/12_location.js";
import * as os from "ext:runtime/30_os.js";
import * as timers from "ext:deno_web/02_timers.js";
import * as performance from "ext:deno_web/15_performance.js";
import {
  denoNsUnstable,
  denoNsUnstableById,
  unstableIds,
} from "ext:runtime/90_deno_ns.js";
import { windowOrWorkerGlobalScope } from "ext:runtime/98_global_scope_shared.js";
import { mainRuntimeGlobalProperties } from "ext:runtime/98_global_scope_window.js";
import {
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
} from "ext:runtime/99_main_shared.js";

let windowIsClosing = false;

function windowClose() {
  if (!windowIsClosing) {
    windowIsClosing = true;
    // Push a macrotask to exit after a promise resolve.
    // This is not perfect, but should be fine for first pass.
    PromisePrototypeThen(
      PromiseResolve(),
      () =>
        FunctionPrototypeCall(timers.setTimeout, null, () => {
          // This should be fine, since only Window/MainWorker has .close()
          os.exit(0);
        }, 0),
    );
  }
}

function opMainModule() {
  return ops.op_main_module();
}

let hasBootstrapped = false;
// Delete the `console` object that V8 automaticaly adds onto the global wrapper
// object on context creation. We don't want this console object to shadow the
// `console` object exposed by the ext/node globalThis proxy.
delete globalThis.console;
// Set up global properties shared by main and worker runtime.
ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);

function bootstrapMainRuntime(runtimeOptions) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }
  const nodeBootstrap = globalThis.nodeBootstrap;

  const {
    0: location_,
    1: unstableFlag,
    2: unstableFeatures,
    3: inspectFlag,
    5: hasNodeModulesDir,
    6: maybeBinaryNpmCommandName,
  } = runtimeOptions;

  performance.setTimeOrigin(DateNow());
  saveGlobalThisReference(globalThis);

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  delete globalThis.nodeBootstrap;
  hasBootstrapped = true;

  // If the `--location` flag isn't set, make `globalThis.location` `undefined` and
  // writable, so that they can mock it themselves if they like. If the flag was
  // set, define `globalThis.location`, using the provided value.
  if (location_ == null) {
    mainRuntimeGlobalProperties.location = {
      writable: true,
    };
  } else {
    location.setLocationHref(location_);
  }

  exposeUnstableFeaturesForWindowOrWorkerGlobalScope({
    unstableFlag,
    unstableFeatures,
  });
  ObjectDefineProperties(globalThis, mainRuntimeGlobalProperties);
  ObjectDefineProperties(globalThis, {
    // TODO(bartlomieju): in the future we might want to change the
    // behavior of setting `name` to actually update the process name.
    // Empty string matches what browsers do.
    name: util.writable(""),
    close: util.writable(windowClose),
    closed: util.getterOnly(() => windowIsClosing),
  });
  ObjectSetPrototypeOf(globalThis, Window.prototype);

  if (inspectFlag) {
    const consoleFromDeno = globalThis.console;
    core.wrapConsole(consoleFromDeno, core.v8Console);
  }

  event.setEventTargetData(globalThis);
  event.saveGlobalThisReference(globalThis);

  event.defineEventHandler(globalThis, "error");
  event.defineEventHandler(globalThis, "load");
  event.defineEventHandler(globalThis, "beforeunload");
  event.defineEventHandler(globalThis, "unload");
  event.defineEventHandler(globalThis, "unhandledrejection");

  runtimeStart(
    denoVersion,
    v8Version,
    tsVersion,
    target,
  );

  ObjectDefineProperties(finalDenoNs, {
    pid: util.getterOnly(opPid),
    ppid: util.getterOnly(opPpid),
    noColor: util.getterOnly(() => ops.op_bootstrap_no_color()),
    args: util.getterOnly(opArgs),
    mainModule: util.getterOnly(opMainModule),
  });

  // TODO(bartlomieju): deprecate --unstable
  if (unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
    // TODO(bartlomieju): this is not ideal, but because we use `ObjectAssign`
    // above any properties that are defined elsewhere using `Object.defineProperty`
    // are lost.
    let jupyterNs = undefined;
    ObjectDefineProperty(finalDenoNs, "jupyter", {
      get() {
        if (jupyterNs) {
          return jupyterNs;
        }
        throw new Error(
          "Deno.jupyter is only available in `deno jupyter` subcommand.",
        );
      },
      set(val) {
        jupyterNs = val;
      },
    });
  } else {
    for (let i = 0; i <= unstableFeatures.length; i++) {
      const id = unstableFeatures[i];
      ObjectAssign(finalDenoNs, denoNsUnstableById[id]);
    }
  }

  if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.unsafeProto)) {
    // Removes the `__proto__` for security reasons.
    // https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
    delete Object.prototype.__proto__;
  }

  // Setup `Deno` global - we're actually overriding already existing global
  // `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));

  if (nodeBootstrap) {
    nodeBootstrap(hasNodeModulesDir, maybeBinaryNpmCommandName);
  }
}

globalThis.bootstrap ??= {};
globalThis.bootstrap.mainRuntime = bootstrapMainRuntime;
