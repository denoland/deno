// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Closer } from "./io.ts";
import { close } from "./ops/resources.ts";
import { lookup, call } from "./ops/ffi.ts";
import {
  loadForeignFunction as opLoadFunction,
  loadForeignLibrary as opLoadLibrary,
} from "./ops/ffi.ts";
import { MemoryAddr, ForeignFunctionInfo } from "./ops/ffi.ts";
export { MemoryAddr, ForeignFunctionInfo } from "./ops/ffi.ts";

export class ForeignLibrary implements Closer {
  constructor(readonly rid: number) {}
  lookup(symbol: string): MemoryAddr {
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
    return call(this.rid, args);
  }
  close(): void {
    close(this.rid);
  }
}

// Those should be generated at compile time, but how?
export const foreignABIs: string[] = [
  /* TODO */
];
export const foreignTypes: string[] = [
  /* TODO */
];

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
