// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This is a "special" module, in that it define the global runtime scope of
// Deno, and therefore it defines a lot of the runtime environment that code
// is evaluated in.

// By convention we import those items that are globally exposed as namespaces
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

// This global augmentation is just enough types to be able to build Deno,
// the runtime types are fully defined in `lib.deno_runtime.d.ts`.
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

  interface EvalErrorInfo {
    isNativeError: boolean;
    isCompileError: boolean;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    thrown: any;
  }

  interface DenoCore {
    print(s: string, isErr?: boolean): void;
    dispatch(
      opId: number,
      control: Uint8Array,
      zeroCopy?: ArrayBufferView | null
    ): Uint8Array | null;
    setAsyncHandler(opId: number, cb: (msg: Uint8Array) => void): void;
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

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    evalContext(code: string): [any, EvalErrorInfo | null];

    errorToJSON: (e: Error) => string;
  }

  // Only `var` variables show up in the `globalThis` type when doing a global
  // scope augmentation.
  /* eslint-disable no-var */
  var addEventListener: (
    type: string,
    callback: (event: domTypes.Event) => void | null,
    options?: boolean | domTypes.AddEventListenerOptions | undefined
  ) => void;
  var compilerMain: (() => void) | undefined;
  var console: consoleTypes.Console;
  var Deno: {
    core: DenoCore;
  };
  var denoMain: (() => void) | undefined;
  var location: domTypes.Location;
  var onerror:
    | ((
        msg: string,
        source: string,
        lineno: number,
        colno: number,
        e: domTypes.Event
      ) => boolean | void)
    | undefined;
  var onload: ((e: domTypes.Event) => void) | undefined;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  var onmessage: ((e: { data: any }) => Promise<void> | void) | undefined;
  var onunload: ((e: domTypes.Event) => void) | undefined;
  var queueMicrotask: (callback: () => void) => void;
  var wasmCompilerMain: (() => void) | undefined;
  var workerMain: (() => Promise<void> | void) | undefined;
  /* eslint-enable */
}

// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// @ts-ignore
Deno[Deno.symbols.internal] = internalObject;

function writable(value: unknown): PropertyDescriptor {
  return {
    value,
    writable: true,
    enumerable: true,
    configurable: true
  };
}

function nonEnumerable(value: unknown): PropertyDescriptor {
  return {
    value,
    writable: true,
    configurable: true
  };
}

function readOnly(value: unknown): PropertyDescriptor {
  return {
    value,
    enumerable: true
  };
}

const globalProperties = {
  window: readOnly(globalThis),
  Deno: readOnly(Deno),
  atob: writable(textEncoding.atob),
  btoa: writable(textEncoding.btoa),
  fetch: writable(fetchTypes.fetch),
  clearTimeout: writable(timers.clearTimeout),
  clearInterval: writable(timers.clearInterval),
  console: writable(new consoleTypes.Console(core.print)),
  setTimeout: writable(timers.setTimeout),
  setInterval: writable(timers.setInterval),
  onload: writable(undefined),
  onunload: writable(undefined),
  crypto: readOnly(csprng),
  Blob: nonEnumerable(blob.DenoBlob),
  File: nonEnumerable(domFile.DomFileImpl),
  CustomEvent: nonEnumerable(customEvent.CustomEvent),
  Event: nonEnumerable(event.Event),
  EventTarget: nonEnumerable(eventTarget.EventTarget),
  URL: nonEnumerable(url.URL),
  URLSearchParams: nonEnumerable(urlSearchParams.URLSearchParams),
  Headers: nonEnumerable(headers.Headers),
  FormData: nonEnumerable(formData.FormData),
  TextEncoder: nonEnumerable(textEncoding.TextEncoder),
  TextDecoder: nonEnumerable(textEncoding.TextDecoder),
  Request: nonEnumerable(request.Request),
  Response: nonEnumerable(fetchTypes.Response),
  performance: writable(new performanceUtil.Performance()),

  onmessage: writable(workers.onmessage),
  onerror: writable(workers.onerror),

  workerMain: nonEnumerable(workers.workerMain),
  workerClose: nonEnumerable(workers.workerClose),
  postMessage: writable(workers.postMessage),
  Worker: nonEnumerable(workers.WorkerImpl),

  [domTypes.eventTargetHost]: nonEnumerable(null),
  [domTypes.eventTargetListeners]: nonEnumerable({}),
  [domTypes.eventTargetMode]: nonEnumerable(""),
  [domTypes.eventTargetNodeType]: nonEnumerable(0),
  [eventTarget.eventTargetAssignedSlot]: nonEnumerable(false),
  [eventTarget.eventTargetHasActivationBehavior]: nonEnumerable(false),

  addEventListener: readOnly(
    eventTarget.EventTarget.prototype.addEventListener
  ),
  dispatchEvent: readOnly(eventTarget.EventTarget.prototype.dispatchEvent),
  removeEventListener: readOnly(
    eventTarget.EventTarget.prototype.removeEventListener
  )
};

Object.defineProperties(globalThis, globalProperties);

// Registers the handler for window.onload function.
globalThis.addEventListener("load", (e: domTypes.Event): void => {
  const { onload } = globalThis;
  if (typeof onload === "function") {
    onload(e);
  }
});
// Registers the handler for window.onunload function.
globalThis.addEventListener("unload", (e: domTypes.Event): void => {
  const { onunload } = globalThis;
  if (typeof onunload === "function") {
    onunload(e);
  }
});
