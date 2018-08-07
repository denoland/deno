// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { Console } from "./console";
import { RawSourceMap } from "./types";
import * as timers from "./timers";
import { TextEncoder, TextDecoder } from "./text_encoding";
import { fetch } from "./fetch";

declare global {
  interface Window {
    console: Console;
  }

  const clearTimeout: typeof timers.clearTimer;
  const clearInterval: typeof timers.clearTimer;
  const setTimeout: typeof timers.setTimeout;
  const setInterval: typeof timers.setInterval;

  const console: Console;
  const window: Window;

  // tslint:disable:variable-name
  let TextEncoder: TextEncoder;
  let TextDecoder: TextDecoder;
  // tslint:enable:variable-name
}

// If you use the eval function indirectly, by invoking it via a reference
// other than eval, as of ECMAScript 5 it works in the global scope rather than
// the local scope. This means, for instance, that function declarations create
// global functions, and that the code being evaluated doesn't have access to
// local variables within the scope where it's being called.
export const globalEval = eval;

// A reference to the global object.
export const window = globalEval("this");
window.window = window;

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;
interface Libdeno {
  recv(cb: MessageCallback): void;
  send(msg: ArrayBufferView): null | Uint8Array;
  print(x: string): void;
  mainSource: string;
  mainSourceMap: RawSourceMap;
}
export const libdeno = window.libdeno as Libdeno;
window.libdeno = null;

// import "./url";

window.setTimeout = timers.setTimeout;
window.setInterval = timers.setInterval;
window.clearTimeout = timers.clearTimer;
window.clearInterval = timers.clearTimer;

window.console = new Console(libdeno.print);
window.TextEncoder = TextEncoder;
window.TextDecoder = TextDecoder;

window.fetch = fetch;
