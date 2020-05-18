// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function lookup(rid: number, symbol: string): number {
  return sendSync("op_dlsym", {
    rid,
    symbol,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function call(rid: number, args: any[]): any {
  return sendSync("op_ffi_call", {
    rid,
    args,
  });
}

export function readMemory(addr: MemoryAddr, p: Uint8Array): void {
  // TODO implement this
}

export function writeMemory(addr: MemoryAddr, p: Uint8Array): void {
  // TODO implement this
}

export interface ForeignFunctionInfo {
  ret: string;
  args: string[];
  numVariadic: number | null;
}

export function loadForeignLibrary(path: string | null): number {
  return sendSync("op_dlopen", {
    path,
  });
}

export function loadForeignFunction(
  addr: number,
  abi: string,
  info: ForeignFunctionInfo
): number {
  return sendSync("op_ffi_prep_cif", {
    addr,
    abi,
    ...info,
  });
}
