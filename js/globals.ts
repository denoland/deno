// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

// If you use the eval function indirectly, by invoking it via a reference
// other than eval, as of ECMAScript 5 it works in the global scope rather than
// the local scope. This means, for instance, that function declarations create
// global functions, and that the code being evaluated doesn't have access to
// local variables within the scope where it's being called.
export const globalEval = eval;

// A reference to the global object.
// TODO The underscore is because it's conflicting with @types/node.
export const window = globalEval("this");

window["window"] = window; // Create a window object.
// import "./url";

// import * as timer from "./timers";
// window["setTimeout"] = timer.setTimeout;
// window["setInterval"] = timer.setInterval;
// window["clearTimeout"] = timer.clearTimer;
// window["clearInterval"] = timer.clearTimer;

import { Console } from "./console";
window["console"] = new Console();

// import { fetch } from "./fetch";
// window["fetch"] = fetch;

// import { TextEncoder, TextDecoder } from "text-encoding";
// window["TextEncoder"] = TextEncoder;
// window["TextDecoder"] = TextDecoder;
