// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import * as timer from "./timers";

// If you use the eval function indirectly, by invoking it via a reference
// other than eval, as of ECMAScript 5 it works in the global scope rather than
// the local scope. This means, for instance, that function declarations create
// global functions, and that the code being evaluated doesn't have access to
// local variables within the scope where it's being called.
export const globalEval = eval;

// A reference to the global object.
// TODO The underscore is because it's conflicting with @types/node.
export const _global = globalEval("this");

_global["window"] = _global; // Create a window object.
import "./url";

_global["setTimeout"] = timer.setTimeout;
_global["setInterval"] = timer.setInterval;
_global["clearTimeout"] = timer.clearTimer;
_global["clearInterval"] = timer.clearTimer;

const print = V8Worker2.print;

_global["console"] = {
  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    print(stringifyArgs(args));
  },

  // tslint:disable-next-line:no-any
  error(...args: any[]): void {
    print("ERROR: " + stringifyArgs(args));
  }
};

// tslint:disable-next-line:no-any
function stringifyArgs(args: any[]): string {
  const out: string[] = [];
  for (const a of args) {
    if (typeof a === "string") {
      out.push(a);
    } else {
      out.push(JSON.stringify(a));
    }
  }
  return out.join(" ");
}

import { fetch } from "./fetch";
_global["fetch"] = fetch;

import { TextEncoder, TextDecoder } from "text-encoding";
_global["TextEncoder"] = TextEncoder;
_global["TextDecoder"] = TextDecoder;
