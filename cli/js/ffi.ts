// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Closer } from "./io.ts";
import { close } from "./ops/resources.ts";
import {
  lookup,
  call,
  loadForeignFunction as opLoadFunction,
  loadForeignLibrary as opLoadLibrary,
} from "./ops/ffi.ts";
export { listForeignABIs, bufferStart, bufferFromPointer } from "./ops/ffi.ts";
import { ForeignFunctionInfo } from "./ops/ffi.ts";
export { ForeignType, ForeignFunctionInfo } from "./ops/ffi.ts";

export class ForeignLibrary implements Closer {
  constructor(readonly rid: number) {}
  lookup(symbol: string): BigInt {
    return lookup(this.rid, symbol);
  }
  close(): void {
    close(this.rid);
  }
}

export class ForeignFunction implements Closer {
  constructor(readonly rid: number) {}
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  call(...args: any[]): any {
    // TODO sanitize and type conversion on call and return
    return call(this.rid, args);
  }
  close(): void {
    close(this.rid);
  }
}

export function loadForeignLibrary(path: string | null): ForeignLibrary {
  const rid = opLoadLibrary(path);
  return new ForeignLibrary(rid);
}

export function loadForeignFunction(
  addr: number,
  abi: string,
  info: ForeignFunctionInfo
): ForeignFunction {
  const rid = opLoadFunction(addr, abi, info);
  return new ForeignFunction(rid);
}
