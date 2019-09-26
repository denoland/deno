// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// This is a "special" module, in that it define the global runtime scope of
// Deno, and therefore it defines a lot of the runtime environment that code
// is evaluated in.  We use this file to automatically build the runtime type
// library.

// Modules which will make up part of the global public API surface should be
// imported as namespaces, so when the runtime type library is generated they
// can be expressed as a namespace in the type library.
import { window } from "./window.ts";
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
}

// A self reference to the global object.
window.window = window;

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
window.location = (undefined as unknown) as domTypes.Location;
window.onload = undefined as undefined | Function;
// The following Crypto interface implementation is not up to par with the
// standard https://www.w3.org/TR/WebCryptoAPI/#crypto-interface as it does not
// yet incorporate the SubtleCrypto interface as its "subtle" property.
window.crypto = (csprng as unknown) as Crypto;
// window.queueMicrotask added by hand to self-maintained lib.deno_runtime.d.ts

// When creating the runtime type library, we use modifications to `window` to
// determine what is in the global namespace.  When we put a class in the
// namespace, we also need its global instance type as well, otherwise users
// won't be able to refer to instances.
// We have to export the type aliases, so that TypeScript _knows_ they are
// being used, which it cannot statically determine within this module.
window.Blob = blob.DenoBlob;
export type Blob = domTypes.Blob;

export type Body = domTypes.Body;

window.File = domFile.DomFileImpl as domTypes.DomFileConstructor;
export type File = domTypes.DomFile;

export type CustomEventInit = domTypes.CustomEventInit;
window.CustomEvent = customEvent.CustomEvent;
export type CustomEvent = domTypes.CustomEvent;
export type EventInit = domTypes.EventInit;
window.Event = event.Event;
export type Event = domTypes.Event;
export type EventListener = domTypes.EventListener;
window.EventTarget = eventTarget.EventTarget;
export type EventTarget = domTypes.EventTarget;
window.URL = url.URL;
export type URL = url.URL;
window.URLSearchParams = urlSearchParams.URLSearchParams;
export type URLSearchParams = domTypes.URLSearchParams;

// Using the `as` keyword to use standard compliant interfaces as the Deno
// implementations contain some implementation details we wouldn't want to
// expose in the runtime type library.
window.Headers = headers.Headers as domTypes.HeadersConstructor;
export type Headers = domTypes.Headers;
window.FormData = formData.FormData as domTypes.FormDataConstructor;
export type FormData = domTypes.FormData;

window.TextEncoder = textEncoding.TextEncoder;
export type TextEncoder = textEncoding.TextEncoder;
window.TextDecoder = textEncoding.TextDecoder;
export type TextDecoder = textEncoding.TextDecoder;

window.Request = request.Request as domTypes.RequestConstructor;
export type Request = domTypes.Request;

window.Response = fetchTypes.Response;
export type Response = domTypes.Response;

window.performance = new performanceUtil.Performance();

// This variable functioning correctly depends on `declareAsLet`
// in //tools/ts_library_builder/main.ts
window.onmessage = workers.onmessage;

window.workerMain = workers.workerMain;
window.workerClose = workers.workerClose;
window.postMessage = workers.postMessage;

window.Worker = workers.WorkerImpl;
export type Worker = workers.Worker;

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
window.addEventListener(
  "load",
  (e: domTypes.Event): void => {
    const onload = window.onload;
    if (typeof onload === "function") {
      onload(e);
    }
  }
);

// below are interfaces that are available in TypeScript but
// have different signatures
export interface ImportMeta {
  url: string;
  main: boolean;
}

export interface Crypto {
  readonly subtle: null;
  getRandomValues: <
    T extends
      | Int8Array
      | Uint8Array
      | Uint8ClampedArray
      | Int16Array
      | Uint16Array
      | Int32Array
      | Uint32Array
  >(
    typedArray: T
  ) => T;
}
