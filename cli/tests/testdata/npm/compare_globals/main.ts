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
