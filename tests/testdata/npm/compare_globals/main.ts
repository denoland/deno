/// <reference types="npm:@types/node" />

import * as globals from "npm:@denotest/globals";
console.log(globals.global === globals.globalThis);
// @ts-expect-error even though these are the same object, they have different types
console.log(globals.globalThis === globalThis);
console.log(globals.process.execArgv);

type AssertTrue<T extends true> = never;
type _TestNoProcessGlobal = AssertTrue<
  typeof globalThis extends { process: any } ? false : true
>;
type _TestHasNodeJsGlobal = NodeJS.Architecture;

const controller = new AbortController();
controller.abort("reason"); // in the NodeJS declaration it doesn't have a reason

// Some globals are not the same between Node and Deno.
// @ts-expect-error incompatible types between Node and Deno
console.log(globalThis.setTimeout === globals.getSetTimeout());

// Super edge case where some Node code deletes a global where the
// Node code has its own global and the Deno code has the same global,
// but it's different. Basically if some Node code deletes
// one of these globals then we don't want it to suddenly inherit
// the Deno global (or touch the Deno global at all).
console.log(typeof globalThis.setTimeout);
console.log(typeof globals.getSetTimeout());
globals.deleteSetTimeout();
console.log(typeof globalThis.setTimeout);
console.log(typeof globals.getSetTimeout());

// In Deno, the process global is not defined, but in Node it is.
console.log("process" in globalThis);
console.log(
  Object.getOwnPropertyDescriptor(globalThis, "process") !== undefined,
);
globals.checkProcessGlobal();

// In Deno, the window global is defined, but in Node it is not.
console.log("window" in globalThis);
console.log(
  Object.getOwnPropertyDescriptor(globalThis, "window") !== undefined,
);
globals.checkWindowGlobal();

// "Non-managed" globals are shared between Node and Deno.
(globalThis as any).foo = "bar";
console.log((globalThis as any).foo);
console.log(globals.getFoo());

console.log(Reflect.ownKeys(globalThis).includes("console")); // non-enumerable keys are included
