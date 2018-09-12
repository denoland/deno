// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { Console } from "./console";
import * as timers from "./timers";
import * as textEncoding from "./text_encoding";
import * as fetch_ from "./fetch";
import { libdeno } from "./libdeno";
import { globalEval } from "./global-eval";

declare global {
  interface Window {
    define: Readonly<unknown>;

    clearTimeout: typeof clearTimeout;
    clearInterval: typeof clearInterval;
    setTimeout: typeof setTimeout;
    setInterval: typeof setInterval;

    console: typeof console;
    window: typeof window;

    fetch: typeof fetch;

    TextEncoder: typeof TextEncoder;
    TextDecoder: typeof TextDecoder;
  }

  const clearTimeout: typeof timers.clearTimer;
  const clearInterval: typeof timers.clearTimer;
  const setTimeout: typeof timers.setTimeout;
  const setInterval: typeof timers.setInterval;

  const console: Console;
  const window: Window;

  const fetch: typeof fetch_.fetch;

  // tslint:disable:variable-name
  const TextEncoder: typeof textEncoding.TextEncoder;
  const TextDecoder: typeof textEncoding.TextDecoder;
  // tslint:enable:variable-name
}

// A reference to the global object.
export const window = globalEval("this");
window.window = window;

window.libdeno = null;

window.setTimeout = timers.setTimeout;
window.setInterval = timers.setInterval;
window.clearTimeout = timers.clearTimer;
window.clearInterval = timers.clearTimer;

window.console = new Console(libdeno.print);
window.TextEncoder = textEncoding.TextEncoder;
window.TextDecoder = textEncoding.TextDecoder;

window.fetch = fetch_.fetch;
