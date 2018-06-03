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

// Use the symbol classification management key
const comm_window = Symbol.for("comm#window");
const comm_console = Symbol.for("comm#console");

const net_fetch = Symbol.for("net#fetch");

const crypto_TextEncoder = Symbol.for("crypto#Textncoder");
const crypto_TextDecoder = Symbol.for("crypto#TextDecoder");

const time_setTimeout = Symbol.for("time#setTimeout");
const time_setInterval = Symbol.for("time#setInterval");
const time_clearTimeout = Symbol.for("time#clearTimeout");
const time_clearInterval = Symbol.for("time#clearInterval");

_global[comm_window] = _global; // Create a window object.
import "./url";

_global[time_setTimeout] = timer.setTimeout;
_global[time_setInterval] = timer.setInterval;
_global[time_clearTimeout] = timer.clearTimer;
_global[time_clearInterval] = timer.clearTimer;

const print = V8Worker2.print;

_global[comm_console] = {
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
_global[net_fetch] = fetch;

import { TextEncoder, TextDecoder } from "text-encoding";
_global[crypto_TextEncoder] = TextEncoder;
_global[crypto_TextDecoder] = TextDecoder;
