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

import { Console } from "./console";
_global["console"] = new Console();

import { fetch } from "./fetch";
_global["fetch"] = fetch;

import { TextEncoder, TextDecoder } from "text-encoding";
_global["TextEncoder"] = TextEncoder;
_global["TextDecoder"] = TextDecoder;
