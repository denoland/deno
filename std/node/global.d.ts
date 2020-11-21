// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { process as processModule } from "./process.ts";
import { Buffer as bufferModule } from "./buffer.ts";

// d.ts files allow us to declare Buffer as a value and as a type
// type something = Buffer | something_else; is quite common

type GlobalType = {
  process: typeof processModule;
  Buffer: typeof bufferModule;
};

declare global {
  interface Window {
    global: GlobalType;
  }

  interface globalThis {
    global: GlobalType;
  }

  var global: GlobalType;
  var process: typeof processModule;
  var Buffer: typeof bufferModule;
  type Buffer = bufferModule;
}

export {};
