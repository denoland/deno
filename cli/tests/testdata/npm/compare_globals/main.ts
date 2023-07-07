/// <reference types="npm:@types/node" />

import * as globals from "npm:@denotest/globals";
console.log(globals.global === globals.globalThis);
console.log(globals.process.execArgv);

type AssertTrue<T extends true> = never;
type _TestNoProcessGlobal = AssertTrue<
  typeof globalThis extends { process: any } ? false : true
>;
type _TestHasNodeJsGlobal = NodeJS.Architecture;

const controller = new AbortController();
controller.abort("reason"); // in the NodeJS declaration it doesn't have a reason

// Super edge case where some Node code deletes a global where the
// Node code has its own global and the Deno code has the same global,
// but it's different. Basically if some Node code deletes
// one of these globals then we don't want it to suddenly inherit
// the Deno global.
globals.withNodeGlobalThis((nodeGlobalThis: any) => {
  (globalThis as any).setTimeout = 5;
  console.log(setTimeout);
  delete nodeGlobalThis["setTimeout"];
  console.log(nodeGlobalThis["setTimeout"]); // should be undefined
  console.log(globalThis["setTimeout"]); // should be undefined
});
