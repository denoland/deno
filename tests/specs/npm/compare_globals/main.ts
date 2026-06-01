/// <reference types="npm:@types/node" />

import { writeFile } from "node:fs";
import * as globals from "npm:@denotest/globals";
console.log(globals.global === globals.globalThis);
console.log(globals.globalThis === globalThis);
console.log(globals.process.execArgv);
console.log("process equals process", process === globals.process);

type AssertTrue<T extends true> = never;
type _TestHasProcessGlobal = AssertTrue<
  typeof globalThis extends { process: any } ? true : false
>;
type _TestProcessGlobalVersion = AssertTrue<
  typeof process.versions.node extends string ? true : false
>;
type _TestHasBufferGlobal = AssertTrue<
  typeof globalThis extends { Buffer: any } ? true : false
>;
type _TestHasNodeJsGlobal = NodeJS.Architecture;

// Regression test for https://github.com/denoland/deno/issues/27150
// The Deno `RequestInit`/`ResponseInit` globals must take precedence over the
// ones declared by `@types/node` (the latter can resolve to an empty
// interface). Otherwise web-standard properties such as `signal` would not
// exist on the `fetch` init type. The reproduction from the issue:
const _issue27150 =
  (_rawFetch: typeof globalThis.fetch) =>
  async (...args: Parameters<typeof globalThis.fetch>) => {
    const { signal: _userSignal } = args[1] ?? {};
    return _userSignal;
  };
type _TestRequestInitHasSignal = AssertTrue<
  "signal" extends keyof NonNullable<Parameters<typeof globalThis.fetch>[1]>
    ? true
    : false
>;
type _TestResponseInitHasStatus = AssertTrue<
  "status" extends keyof NonNullable<
    ConstructorParameters<typeof Response>[1]
  > ? true
    : false
>;

const controller = new AbortController();
controller.abort("reason"); // in the NodeJS declaration it doesn't have a reason

// Regression test for https://github.com/denoland/deno/issues/19527
// The `AbortSignal` produced by Deno's `AbortController` must be assignable
// to the `AbortSignal` parameter of `@types/node` APIs. Historically Deno
// kept its own `AbortController`/`AbortSignal` in a separate Node-only
// global table; that caused TS2300/TS2320 duplicate-identifier errors and
// callers to see two incompatible `AbortSignal` types here.
const _issue19527 = () =>
  writeFile(
    "file.txt",
    "content",
    { signal: controller.signal },
    (_err) => {},
  );
// `AbortSignal` must remain an `EventTarget` subtype. The original issue
// hit TS2320 because `interface AbortSignal extends EventTarget` was being
// declared by both Deno and `@types/node`, with conflicting `dispatchEvent`
// signatures.
const _signalIsEventTarget: EventTarget = controller.signal;
// `AbortSignal.timeout` is a Deno-provided static. It must still resolve to
// `AbortSignal` (not a `@types/node` shadow type) once both libs are loaded.
const _signalTimeoutTypeCheck: AbortSignal = AbortSignal.timeout(0);

// Some globals are not the same between Node and Deno.
console.log("setTimeout 1", globalThis.setTimeout === globals.getSetTimeout());

// Super edge case where some Node code deletes a global where the
// Node code has its own global and the Deno code has the same global,
// but it's different. Basically if some Node code deletes
// one of these globals then we don't want it to suddenly inherit
// the Deno global (or touch the Deno global at all).
console.log("setTimeout 2", typeof globalThis.setTimeout);
console.log("setTimeout 3", typeof globals.getSetTimeout());
globals.deleteSetTimeout();
console.log("setTimeout 4", typeof globalThis.setTimeout);
console.log("setTimeout 5", typeof globals.getSetTimeout());

// In Deno 2 and Node.js, the window global is not defined.
console.log("window 1", "window" in globalThis);
console.log(
  "window 2",
  Object.getOwnPropertyDescriptor(globalThis, "window") !== undefined,
);
globals.checkWindowGlobal();

// In Deno 2 self global is defined, but in Node it is not.
console.log("self 1", "self" in globalThis);
console.log(
  "self 2",
  Object.getOwnPropertyDescriptor(globalThis, "self") !== undefined,
);
globals.checkSelfGlobal();

// "Non-managed" globals are shared between Node and Deno.
(globalThis as any).foo = "bar";
console.log((globalThis as any).foo);
console.log(globals.getFoo());

console.log(Reflect.ownKeys(globalThis).includes("console")); // non-enumerable keys are included
