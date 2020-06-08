// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import "./lib.deno.shared_globals.d.ts";

import * as abortController from "./web/abort_controller.ts";
import * as abortSignal from "./web/abort_signal.ts";
import * as blob from "./web/blob.ts";
import * as consoleTypes from "./web/console.ts";
import * as csprng from "./ops/get_random_values.ts";
import * as promiseTypes from "./web/promise.ts";
import * as customEvent from "./web/custom_event.ts";
import * as domException from "./web/dom_exception.ts";
import * as domFile from "./web/dom_file.ts";
import * as errorEvent from "./web/error_event.ts";
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
import * as readableStream from "./web/streams/readable_stream.ts";
import * as transformStream from "./web/streams/transform_stream.ts";
import * as queuingStrategy from "./web/streams/queuing_strategy.ts";
import * as writableStream from "./web/streams/writable_stream.ts";

// These imports are not exposed and therefore are fine to just import the
// symbols required.
import { core } from "./core.ts";

// This global augmentation is just enough types to be able to build Deno,
// the runtime types are fully defined in `lib.deno.*.d.ts`.
declare global {
  interface CallSite {
    getThis(): unknown;
    getTypeName(): string | null;
    getFunction(): Function | null;
    getFunctionName(): string | null;
    getMethodName(): string | null;
    getFileName(): string | null;
    getLineNumber(): number | null;
    getColumnNumber(): number | null;
    getEvalOrigin(): string | null;
    isToplevel(): boolean | null;
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
      ...zeroCopy: ArrayBufferView[]
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
      ...data: ArrayBufferView[]
    ): null | Uint8Array;

    setMacrotaskCallback(cb: () => boolean): void;

    shared: SharedArrayBuffer;

    evalContext(
      code: string,
      scriptName?: string
    ): [unknown, EvalErrorInfo | null];

    formatError: (e: Error) => string;

    /**
     * Get promise details as two elements array.
     *
     * First element is the `PromiseState`.
     * If promise isn't pending, second element would be the result of the promise.
     * Otherwise, second element would be undefined.
     *
     * Throws `TypeError` if argument isn't a promise
     *
     */
    getPromiseDetails<T>(promise: Promise<T>): promiseTypes.PromiseDetails<T>;

    decode(bytes: Uint8Array): string;
    encode(text: string): Uint8Array;
  }

  // Only `var` variables show up in the `globalThis` type when doing a global
  // scope augmentation.
  /* eslint-disable no-var */

  // Assigned to `window` global - main runtime
  var Deno: {
    core: DenoCore;
    noColor: boolean;
  };
  var onload: ((e: Event) => void) | undefined;
  var onunload: ((e: Event) => void) | undefined;

  // These methods are used to prepare different runtime
  // environments. After bootrapping, this namespace
  // should be removed from global scope.
  var bootstrap: {
    mainRuntime: (() => void) | undefined;
    // Assigned to `self` global - worker runtime and compiler
    workerRuntime: ((name: string) => Promise<void> | void) | undefined;
    // Assigned to `self` global - compiler
    tsCompilerRuntime: (() => void) | undefined;
  };

  var onerror:
    | ((
        msg: string,
        source: string,
        lineno: number,
        colno: number,
        e: Event
      ) => boolean | void)
    | undefined;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  var onmessage: ((e: { data: any }) => Promise<void> | void) | undefined;
  // Called in compiler
  var close: () => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  var postMessage: (msg: any) => void;
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
  AbortController: nonEnumerable(abortController.AbortControllerImpl),
  AbortSignal: nonEnumerable(abortSignal.AbortSignalImpl),
  Blob: nonEnumerable(blob.DenoBlob),
  ByteLengthQueuingStrategy: nonEnumerable(
    queuingStrategy.ByteLengthQueuingStrategyImpl
  ),
  CountQueuingStrategy: nonEnumerable(queuingStrategy.CountQueuingStrategyImpl),
  crypto: readOnly(csprng),
  File: nonEnumerable(domFile.DomFileImpl),
  CustomEvent: nonEnumerable(customEvent.CustomEventImpl),
  DOMException: nonEnumerable(domException.DOMExceptionImpl),
  ErrorEvent: nonEnumerable(errorEvent.ErrorEventImpl),
  Event: nonEnumerable(event.EventImpl),
  EventTarget: nonEnumerable(eventTarget.EventTargetImpl),
  URL: nonEnumerable(url.URLImpl),
  URLSearchParams: nonEnumerable(urlSearchParams.URLSearchParamsImpl),
  Headers: nonEnumerable(headers.HeadersImpl),
  FormData: nonEnumerable(formData.FormDataImpl),
  TextEncoder: nonEnumerable(textEncoding.TextEncoder),
  TextDecoder: nonEnumerable(textEncoding.TextDecoder),
  ReadableStream: nonEnumerable(readableStream.ReadableStreamImpl),
  TransformStream: nonEnumerable(transformStream.TransformStreamImpl),
  Request: nonEnumerable(request.Request),
  Response: nonEnumerable(fetchTypes.Response),
  performance: writable(new performanceUtil.Performance()),
  Worker: nonEnumerable(workers.WorkerImpl),
  WritableStream: nonEnumerable(writableStream.WritableStreamImpl),
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function setEventTargetData(value: any): void {
  eventTarget.eventTargetData.set(value, eventTarget.getDefaultTargetData());
}

export const eventTargetProperties = {
  addEventListener: readOnly(
    eventTarget.EventTargetImpl.prototype.addEventListener
  ),
  dispatchEvent: readOnly(eventTarget.EventTargetImpl.prototype.dispatchEvent),
  removeEventListener: readOnly(
    eventTarget.EventTargetImpl.prototype.removeEventListener
  ),
};
