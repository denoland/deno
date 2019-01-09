// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// This is a "special" module, in that it define the global runtime scope of
// Deno, and therefore it defines a lot of the runtime environemnt that code
// is evaluated in.  We use this file to automatically build the runtime type
// library.

// Modules which will make up part of the global public API surface should be
// imported as namespaces, so when the runtime tpye library is generated they
// can be expressed as a namespace in the type library.
import * as blob from "./blob";
import * as consoleTypes from "./console";
import * as domTypes from "./dom_types";
import * as event from "./event";
import * as eventTarget from "./event_target";
import * as formData from "./form_data";
import * as fetchTypes from "./fetch";
import * as headers from "./headers";
import * as textEncoding from "./text_encoding";
import * as timers from "./timers";
import * as url from "./url";
import * as urlSearchParams from "./url_search_params";
import * as workers from "./workers";

// These imports are not exposed and therefore are fine to just import the
// symbols required.
import { globalEval } from "./global_eval";
import { libdeno } from "./libdeno";

// During the build process, augmentations to the variable `window` in this
// file are tracked and created as part of default library that is built into
// Deno, we only need to declare the enough to compile Deno.
declare global {
  const console: consoleTypes.Console;
  const setTimeout: typeof timers.setTimeout;
}

// A reference to the global object.
export const window = globalEval("this");
// A self reference to the global object.
window.window = window;

// Globally available functions and object instances.
window.atob = textEncoding.atob;
window.btoa = textEncoding.btoa;
window.fetch = fetchTypes.fetch;
window.clearTimeout = timers.clearTimer;
window.clearInterval = timers.clearTimer;
window.console = new consoleTypes.Console(libdeno.print);
window.setTimeout = timers.setTimeout;
window.setInterval = timers.setInterval;

// When creating the runtime type library, we use modifications to `window` to
// determine what is in the global namespace.  When we put a class in the
// namespace, we also need its global instance type as well, otherwise users
// won't be able to refer to instances.
// We have to export the type aliases, so that TypeScript _knows_ they are
// being used, which it cannot statically determine within this module.
window.Blob = blob.DenoBlob;
export type Blob = blob.DenoBlob;

// TODO(ry) Do not export a class implementing the DOM, export the DOM
// interface. See this comment for implementation hint:
// https://github.com/denoland/deno/pull/1396#discussion_r243711502
// window.File = file.DenoFile;
// export type File = file.DenoFile;

window.EventInit = event.EventInit;
export type EventInit = event.EventInit;
window.Event = event.Event;
export type Event = event.Event;
window.EventTarget = eventTarget.EventTarget;
export type EventTarget = eventTarget.EventTarget;
window.URL = url.URL;
export type URL = url.URL;
window.URLSearchParams = urlSearchParams.URLSearchParams;
export type URLSearchParams = urlSearchParams.URLSearchParams;

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

window.workerMain = workers.workerMain;
// TODO These shouldn't be available in main isolate.
window.postMessage = workers.postMessage;
window.close = workers.workerClose;
