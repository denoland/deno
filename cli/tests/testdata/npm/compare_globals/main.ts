/// <reference types="npm:@types/node" />

import * as globals from "npm:@denotest/globals";
console.log(globals.global === globals.globalThis);
console.log(globals.process.execArgv);

type AssertTrue<T extends true> = never;
type _TestNoProcessGlobal = AssertTrue<
  typeof globalThis extends { process: any } ? false : true
>;
