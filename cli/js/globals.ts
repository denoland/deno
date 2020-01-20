// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This is a "special" module, in that it define the global runtime scope of
// Deno, and therefore it defines a lot of the runtime environment that code
// is evaluated in.  We use this file to automatically build the runtime type
// library.

// Modules which will make up part of the global public API surface should be
// imported as namespaces, so when the runtime type library is generated they
// can be expressed as a namespace in the type library.
import * as blob from "./blob.ts";
import * as consoleTypes from "./console.ts";
import * as csprng from "./get_random_values.ts";
import * as customEvent from "./custom_event.ts";
import * as Deno from "./deno.ts";
import * as domTypes from "./dom_types.ts";
import * as domFile from "./dom_file.ts";
import * as event from "./event.ts";
import * as eventTarget from "./event_target.ts";
import * as formData from "./form_data.ts";
import * as fetchTypes from "./fetch.ts";
import * as headers from "./headers.ts";
import * as textEncoding from "./text_encoding.ts";
import * as timers from "./timers.ts";
import * as url from "./url.ts";
import * as urlSearchParams from "./url_search_params.ts";
import * as workers from "./workers.ts";
import * as performanceUtil from "./performance.ts";
import * as request from "./request.ts";

// These imports are not exposed and therefore are fine to just import the
// symbols required.
import { core } from "./core.ts";

import { internalObject } from "./internals.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const window = globalThis as any;

interface MessageCallback {
  (msg: Uint8Array): void;
}

interface EvalErrorInfo {
  // Is the object thrown a native Error?
  isNativeError: boolean;
  // Was the error happened during compilation?
  isCompileError: boolean;
  // The actual thrown entity
  // (might be an Error or anything else thrown by the user)
  // If isNativeError is true, this is an Error
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  thrown: any;
}

// During the build process, augmentations to the variable `window` in this
// file are tracked and created as part of default library that is built into
// Deno, we only need to declare the enough to compile Deno.
declare global {
  interface CallSite {
    getThis(): unknown;
    getTypeName(): string;
    getFunction(): Function;
    getFunctionName(): string;
    getMethodName(): string;
    getFileName(): string;
    getLineNumber(): number | null;
    getColumnNumber(): number | null;
    getEvalOrigin(): string | null;
    isToplevel(): boolean;
    isEval(): boolean;
    isNative(): boolean;
    isConstructor(): boolean;
    isAsync(): boolean;
    isPromiseAll(): boolean;
    getPromiseIndex(): number | null;
  }

  interface ErrorConstructor {
    prepareStackTrace(error: Error, structuredStackTrace: CallSite[]): string;
  }

  interface Object {
    [consoleTypes.customInspect]?(): string;
  }

  interface DenoCore {
    print(s: string, isErr?: boolean): void;
    dispatch(
      opId: number,
      control: Uint8Array,
      zeroCopy?: ArrayBufferView | null
    ): Uint8Array | null;
    setAsyncHandler(opId: number, cb: MessageCallback): void;
    sharedQueue: {
      head(): number;
      numRecords(): number;
      size(): number;
      push(buf: Uint8Array): boolean;
      reset(): void;
      shift(): Uint8Array | null;
    };

    ops(): Record<string, number>;

    recv(cb: (opId: number, msg: Uint8Array) => void): void;

    send(
      opId: number,
      control: null | ArrayBufferView,
      data?: ArrayBufferView
    ): null | Uint8Array;

    shared: SharedArrayBuffer;

    /** Evaluate provided code in the current context.
     * It differs from eval(...) in that it does not create a new context.
     * Returns an array: [output, errInfo].
     * If an error occurs, `output` becomes null and `errInfo` is non-null.
     */
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    evalContext(code: string): [any, EvalErrorInfo | null];

    errorToJSON: (e: Error) => string;
  }

  /* eslint-disable no-var */
  var Deno: {
    core: DenoCore;
  };
  var location: domTypes.Location;
  var console: consoleTypes.Console;
  var denoMain: (() => void) | undefined;
  var workerMain: (() => Promise<void> | void) | undefined;
  var compilerMain: (() => void) | undefined;
  var wasmCompilerMain: (() => void) | undefined;
  var queueMicrotask: (callback: () => void) => void;
  var onmessage: ((e: { data: any }) => Promise<void> | void) | undefined;
  var onerror:
    | ((
        msg: string,
        source: string,
        lineno: number,
        colno: number,
        e: domTypes.Event
      ) => boolean | void)
    | undefined;
  /* eslint-enable */
}

// A self reference to the global object.
window.window = window;

// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// @ts-ignore
Deno[Deno.symbols.internal] = internalObject;
// This is the Deno namespace, it is handled differently from other window
// properties when building the runtime type library, as the whole module
// is flattened into a single namespace.
window.Deno = Deno;

// Globally available functions and object instances.
window.atob = textEncoding.atob;
window.btoa = textEncoding.btoa;
window.fetch = fetchTypes.fetch;
window.clearTimeout = timers.clearTimeout;
window.clearInterval = timers.clearInterval;
window.console = new consoleTypes.Console(core.print);
window.setTimeout = timers.setTimeout;
window.setInterval = timers.setInterval;
window.location = undefined;
window.onload = undefined;
window.onunload = undefined;
window.crypto = csprng;
window.Blob = blob.DenoBlob;
window.File = domFile.DomFileImpl;
window.CustomEvent = customEvent.CustomEvent;
window.Event = event.Event;
window.EventTarget = eventTarget.EventTarget;
window.URL = url.URL;
window.URLSearchParams = urlSearchParams.URLSearchParams;
window.Headers = headers.Headers;
window.FormData = formData.FormData;
window.TextEncoder = textEncoding.TextEncoder;
window.TextDecoder = textEncoding.TextDecoder;
window.Request = request.Request;
window.Response = fetchTypes.Response;
window.performance = new performanceUtil.Performance();
window.onmessage = workers.onmessage;
window.onerror = workers.onerror;
window.workerMain = workers.workerMain;
window.workerClose = workers.workerClose;
window.postMessage = workers.postMessage;
window.Worker = workers.WorkerImpl;

window[domTypes.eventTargetHost] = null;
window[domTypes.eventTargetListeners] = {};
window[domTypes.eventTargetMode] = "";
window[domTypes.eventTargetNodeType] = 0;
window[eventTarget.eventTargetAssignedSlot] = false;
window[eventTarget.eventTargetHasActivationBehavior] = false;

window.addEventListener = eventTarget.EventTarget.prototype.addEventListener;
window.dispatchEvent = eventTarget.EventTarget.prototype.dispatchEvent;
window.removeEventListener =
  eventTarget.EventTarget.prototype.removeEventListener;

// Registers the handler for window.onload function.
window.addEventListener("load", (e: domTypes.Event): void => {
  const onload = window.onload;
  if (typeof onload === "function") {
    onload(e);
  }
});
// Registers the handler for window.onunload function.
window.addEventListener("unload", (e: domTypes.Event): void => {
  const onunload = window.onunload;
  if (typeof onunload === "function") {
    onunload(e);
  }
});

// below are interfaces that are available in TypeScript but
// have different signatures
// export interface ImportMeta {
//   url: string;
//   main: boolean;
// }
