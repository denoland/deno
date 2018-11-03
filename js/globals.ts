// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as blob from "./blob";
import * as console_ from "./console";
import * as fetch_ from "./fetch";
import { Headers } from "./headers";
import { globalEval } from "./global_eval";
import { libdeno } from "./libdeno";
import * as textEncoding from "./text_encoding";
import * as timers from "./timers";
import * as urlSearchParams from "./url_search_params";
import * as domTypes from "./dom_types";

// During the build process, augmentations to the variable `window` in this
// file are tracked and created as part of default library that is built into
// deno, we only need to declare the enough to compile deno.

declare global {
  const console: console_.Console;
  const setTimeout: typeof timers.setTimeout;
  // tslint:disable-next-line:variable-name
  const TextEncoder: typeof textEncoding.TextEncoder;
}

// A reference to the global object.
export const window = globalEval("this");
window.window = window;

window.setTimeout = timers.setTimeout;
window.setInterval = timers.setInterval;
window.clearTimeout = timers.clearTimer;
window.clearInterval = timers.clearTimer;

window.console = new console_.Console(libdeno.print);
window.TextEncoder = textEncoding.TextEncoder;
window.TextDecoder = textEncoding.TextDecoder;
window.atob = textEncoding.atob;
window.btoa = textEncoding.btoa;

window.URLSearchParams = urlSearchParams.URLSearchParams;

window.fetch = fetch_.fetch;

// using the `as` keyword to mask the internal types when generating the
// runtime library
window.Headers = Headers as domTypes.HeadersConstructor;
window.Blob = blob.DenoBlob;
