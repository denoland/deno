// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  DateNow,
  Error,
  FunctionPrototypeBind,
  ObjectAssign,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  SymbolIterator,
  TypeError,
} = primordials;
import * as util from "ext:runtime/06_util.js";
import * as event from "ext:deno_web/02_event.js";
import * as location from "ext:deno_web/12_location.js";
import * as os from "ext:runtime/30_os.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as url from "ext:deno_url/00_url.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import {
  denoNsUnstable,
  denoNsUnstableById,
  unstableIds,
} from "ext:runtime/90_deno_ns.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { windowOrWorkerGlobalScope } from "ext:runtime/98_global_scope_shared.js";
import { workerRuntimeGlobalProperties } from "ext:runtime/98_global_scope_worker.js";
import {
  denoVersion,
  exposeUnstableFeaturesForWindowOrWorkerGlobalScope,
  finalDenoNs,
  opArgs,
  opPid,
  runtimeStart,
  saveGlobalThisReference,
  target,
  tsVersion,
  v8Version,
} from "ext:runtime/99_main_shared.js";

function workerClose() {
  if (isClosing) {
    return;
  }

  isClosing = true;
  ops.op_worker_close();
}

function postMessage(message, transferOrOptions = {}) {
  const prefix =
    "Failed to execute 'postMessage' on 'DedicatedWorkerGlobalScope'";
  webidl.requiredArguments(arguments.length, 1, prefix);
  message = webidl.converters.any(message);
  let options;
  if (
    webidl.type(transferOrOptions) === "Object" &&
    transferOrOptions !== undefined &&
    transferOrOptions[SymbolIterator] !== undefined
  ) {
    const transfer = webidl.converters["sequence<object>"](
      transferOrOptions,
      prefix,
      "Argument 2",
    );
    options = { transfer };
  } else {
    options = webidl.converters.StructuredSerializeOptions(
      transferOrOptions,
      prefix,
      "Argument 2",
    );
  }
  const { transfer } = options;
  const data = messagePort.serializeJsMessageData(message, transfer);
  ops.op_worker_post_message(data);
}

let isClosing = false;
let globalDispatchEvent;

async function pollForMessages() {
  const { op_worker_recv_message } = core.ensureFastOps();

  if (!globalDispatchEvent) {
    globalDispatchEvent = FunctionPrototypeBind(
      globalThis.dispatchEvent,
      globalThis,
    );
  }
  while (!isClosing) {
    const data = await op_worker_recv_message();
    if (data === null) break;
    const v = messagePort.deserializeJsMessageData(data);
    const message = v[0];
    const transferables = v[1];

    const msgEvent = new event.MessageEvent("message", {
      cancelable: false,
      data: message,
      ports: ArrayPrototypeFilter(
        transferables,
        (t) =>
          ObjectPrototypeIsPrototypeOf(messagePort.MessagePortPrototype, t),
      ),
    });
    event.setIsTrusted(msgEvent, true);

    try {
      globalDispatchEvent(msgEvent);
    } catch (e) {
      const errorEvent = new event.ErrorEvent("error", {
        cancelable: true,
        message: e.message,
        lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
        colno: e.columnNumber ? e.columnNumber + 1 : undefined,
        filename: e.fileName,
        error: e,
      });

      event.setIsTrusted(errorEvent, true);
      globalDispatchEvent(errorEvent);
      if (!errorEvent.defaultPrevented) {
        throw e;
      }
    }
  }
}

let loadedMainWorkerScript = false;

function importScripts(...urls) {
  if (ops.op_worker_get_type() === "module") {
    throw new TypeError("Can't import scripts in a module worker.");
  }

  const baseUrl = location.getLocationHref();
  const parsedUrls = ArrayPrototypeMap(urls, (scriptUrl) => {
    try {
      return new url.URL(scriptUrl, baseUrl ?? undefined).href;
    } catch {
      throw new DOMException(
        "Failed to parse URL.",
        "SyntaxError",
      );
    }
  });

  // A classic worker's main script has looser MIME type checks than any
  // imported scripts, so we use `loadedMainWorkerScript` to distinguish them.
  // TODO(andreubotella) Refactor worker creation so the main script isn't
  // loaded with `importScripts()`.
  const scripts = ops.op_worker_sync_fetch(
    parsedUrls,
    !loadedMainWorkerScript,
  );
  loadedMainWorkerScript = true;

  for (let i = 0; i < scripts.length; ++i) {
    const { url, script } = scripts[i];
    const err = core.evalContext(script, url)[1];
    if (err !== null) {
      throw err.thrown;
    }
  }
}

let hasBootstrapped = false;
// Delete the `console` object that V8 automaticaly adds onto the global wrapper
// object on context creation. We don't want this console object to shadow the
// `console` object exposed by the ext/node globalThis proxy.
delete globalThis.console;
// Set up global properties shared by main and worker runtime.
ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);

function bootstrapWorkerRuntime(
  runtimeOptions,
  name,
  internalName,
) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }

  const nodeBootstrap = globalThis.nodeBootstrap;

  const {
    0: location_,
    1: unstableFlag,
    2: unstableFeatures,
    4: enableTestingFeaturesFlag,
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

  exposeUnstableFeaturesForWindowOrWorkerGlobalScope({
    unstableFlag,
    unstableFeatures,
  });
  ObjectDefineProperties(globalThis, workerRuntimeGlobalProperties);
  ObjectDefineProperties(globalThis, {
    name: util.writable(name),
    // TODO(bartlomieju): should be readonly?
    close: util.nonEnumerable(workerClose),
    postMessage: util.writable(postMessage),
  });
  if (enableTestingFeaturesFlag) {
    ObjectDefineProperty(
      globalThis,
      "importScripts",
      util.writable(importScripts),
    );
  }
  ObjectSetPrototypeOf(globalThis, DedicatedWorkerGlobalScope.prototype);

  const consoleFromDeno = globalThis.console;
  core.wrapConsole(consoleFromDeno, core.v8Console);

  event.setEventTargetData(globalThis);
  event.saveGlobalThisReference(globalThis);

  event.defineEventHandler(self, "message");
  event.defineEventHandler(self, "error", undefined, true);
  event.defineEventHandler(self, "unhandledrejection");

  // `Deno.exit()` is an alias to `self.close()`. Setting and exit
  // code using an op in worker context is a no-op.
  os.setExitHandler((_exitCode) => {
    workerClose();
  });

  runtimeStart(
    denoVersion,
    v8Version,
    tsVersion,
    target,
    internalName ?? name,
  );

  location.setLocationHref(location_);

  globalThis.pollForMessages = pollForMessages;

  // TODO(bartlomieju): deprecate --unstable
  if (unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
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

  ObjectDefineProperties(finalDenoNs, {
    pid: util.getterOnly(opPid),
    noColor: util.getterOnly(() => ops.op_bootstrap_no_color()),
    args: util.getterOnly(opArgs),
  });
  // Setup `Deno` global - we're actually overriding already
  // existing global `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));

  if (nodeBootstrap) {
    nodeBootstrap(hasNodeModulesDir, maybeBinaryNpmCommandName);
  }
}

globalThis.bootstrap ??= {};
globalThis.bootstrap.workerRuntime = bootstrapWorkerRuntime;
