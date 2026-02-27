// Copyright 2018-2025 the Deno authors. MIT license.
import { asyncNeverResolves } from "checkin:async";

// make a promise that never resolves so we have
// a pending op outstanding
const prom = asyncNeverResolves();

// import a module, with the key being that
// this module promise doesn't resolve until a later
// tick of the event loop
await import("./dynamic.js");
console.log("module imported");

// unref to let the event loop exit
Deno.core.unrefOpPromise(prom);
