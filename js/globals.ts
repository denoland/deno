// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { Console } from "./console";
import { exit } from "./os";
import * as timers from "./timers";
import { TextDecoder, TextEncoder } from "./text_encoding";
import * as fetch_ from "./fetch";
import { libdeno } from "./libdeno";
import { globalEval } from "./global-eval";

declare global {
  interface Window {
    console: Console;
    define: Readonly<unknown>;
    onerror?: (
      message: string,
      source: string,
      lineno: number,
      colno: number,
      error: Error
    ) => void;
  }

  const clearTimeout: typeof timers.clearTimer;
  const clearInterval: typeof timers.clearTimer;
  const setTimeout: typeof timers.setTimeout;
  const setInterval: typeof timers.setInterval;

  const console: Console;
  const window: Window;

  const fetch: typeof fetch_.fetch;

  // tslint:disable:variable-name
  let TextEncoder: TextEncoder;
  let TextDecoder: TextDecoder;
  // tslint:enable:variable-name
}

// A reference to the global object.
export const window = globalEval("this");
window.window = window;

window.libdeno = null;

// import "./url";

window.setTimeout = timers.setTimeout;
window.setInterval = timers.setInterval;
window.clearTimeout = timers.clearTimer;
window.clearInterval = timers.clearTimer;

window.console = new Console(libdeno.print);
// Uncaught exceptions are sent to window.onerror by the privileged binding.
window.onerror = (
  message: string,
  source: string,
  lineno: number,
  colno: number,
  error: Error
) => {
  // TODO Currently there is a bug in v8_source_maps.ts that causes a
  // segfault if it is used within window.onerror. To workaround we
  // uninstall the Error.prepareStackTrace handler. Users will get unmapped
  // stack traces on uncaught exceptions until this issue is fixed.
  //Error.prepareStackTrace = null;
  console.log(error.stack);
  exit(1);
};
window.TextEncoder = TextEncoder;
window.TextDecoder = TextDecoder;

window.fetch = fetch_.fetch;
