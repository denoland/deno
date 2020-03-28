// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as blob from "./web/blob.ts";
import * as consoleTypes from "./web/console.ts";
import * as customEvent from "./web/custom_event.ts";
import * as domTypes from "./web/dom_types.ts";
import * as domFile from "./web/dom_file.ts";
import * as event from "./web/event.ts";
import * as eventTarget from "./web/event_target.ts";
import * as formData from "./web/form_data.ts";
import * as fetchTypes from "./web/fetch.ts";
import * as headers from "./web/headers.ts";
import * as textEncoding from "./web/text_encoding.ts";
import * as timers from "./web/timers.ts";
import * as url from "./web/url.ts";
import * as urlSearchParams from "./web/url_search_params.ts";
import * as workers from "./web/workers.ts";
import * as performanceUtil from "./web/performance.ts";
import * as request from "./web/request.ts";

// These imports are not exposed and therefore are fine to just import the
// symbols required.
import { core } from "./core.ts";

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

  interface ImportMeta {
    url: string;
    main: boolean;
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

    setMacrotaskCallback(cb: () => boolean): void;

    shared: SharedArrayBuffer;

    evalContext(
      code: string,
      scriptName?: string
    ): [unknown, EvalErrorInfo | null];

    formatError: (e: Error) => string;

    decode(bytes: Uint8Array): string;
    encode(text: string): Uint8Array;
  }

  // Only `var` variables show up in the `globalThis` type when doing a global
  // scope augmentation.
  /* eslint-disable no-var */
  var addEventListener: (
    type: string,
    callback: domTypes.EventListenerOrEventListenerObject | null,
    options?: boolean | domTypes.AddEventListenerOptions | undefined
  ) => void;
  var queueMicrotask: (callback: () => void) => void;
  var console: consoleTypes.Console;
  var location: domTypes.Location;

  // Assigned to `window` global - main runtime
  var Deno: {
    core: DenoCore;
  };
  var onload: ((e: domTypes.Event) => void) | undefined;
  var onunload: ((e: domTypes.Event) => void) | undefined;
  var bootstrapMainRuntime: (() => void) | undefined;

  // Assigned to `self` global - worker runtime and compiler
  var bootstrapWorkerRuntime:
    | ((name: string) => Promise<void> | void)
    | undefined;
  var onerror:
    | ((
        msg: string,
        source: string,
        lineno: number,
        colno: number,
        e: domTypes.Event
      ) => boolean | void)
    | undefined;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  var onmessage: ((e: { data: any }) => Promise<void> | void) | undefined;
  // Called in compiler
  var close: () => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  var postMessage: (msg: any) => void;
  // Assigned to `self` global - compiler
  var bootstrapTsCompilerRuntime: (() => void) | undefined;
  var bootstrapWasmCompilerRuntime: (() => void) | undefined;

  var performance: performanceUtil.Performance;
  var setTimeout: typeof timers.setTimeout;
  /* eslint-enable */
}

export function writable(value: unknown): PropertyDescriptor {
  return {
    value,
    writable: true,
    enumerable: true,
    configurable: true,
  };
}

export function nonEnumerable(value: unknown): PropertyDescriptor {
  return {
    value,
    writable: true,
    configurable: true,
  };
}

export function readOnly(value: unknown): PropertyDescriptor {
  return {
    value,
    enumerable: true,
  };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function getterOnly(getter: () => any): PropertyDescriptor {
  return {
    get: getter,
    enumerable: true,
  };
}

// https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
export const windowOrWorkerGlobalScopeMethods = {
  atob: writable(textEncoding.atob),
  btoa: writable(textEncoding.btoa),
  clearInterval: writable(timers.clearInterval),
  clearTimeout: writable(timers.clearTimeout),
  fetch: writable(fetchTypes.fetch),
  // queueMicrotask is bound in Rust
  setInterval: writable(timers.setInterval),
  setTimeout: writable(timers.setTimeout),
};

// Other properties shared between WindowScope and WorkerGlobalScope
export const windowOrWorkerGlobalScopeProperties = {
  console: writable(new consoleTypes.Console(core.print)),
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
  Worker: nonEnumerable(workers.WorkerImpl),
};

export const eventTargetProperties = {
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
  ),
};
