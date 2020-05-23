// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function loadForeignLibrary(path: string | null): number {
  const rid = sendSync("op_dlopen", { path });
  return rid;
}

export function lookup(rid: number, symbol: string): BigInt {
  const addr: string = sendSync("op_dlsym", { rid, symbol });
  return BigInt(addr);
}

export type ForeignType =
  | "void"
  | "uint8"
  | "sint8"
  | "uint16"
  | "sint16"
  | "uint32"
  | "sint32"
  | "uint64"
  | "sint64"
  | "float"
  | "double"
  | "pointer";

export interface ForeignFunctionInfo {
  ret: ForeignType;
  args: ForeignType[];
  numVariadic: number | null;
}

export function loadForeignFunction(
  addr: number,
  abi: string,
  info: ForeignFunctionInfo
): number {
  const rid = sendSync("op_ffi_prep", { addr, abi, ...info });
  return rid;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function call(rid: number, args: any[]): any {
  // TODO BigInt can't fit in JSON
  // return sendSync("op_ffi_call", {
  //   rid,
  //   args,
  // });
}

export function listForeignABIs(): string[] {
  return sendSync("op_ffi_list_abi");
}

export function bufferStart(buffer: SharedArrayBuffer): BigInt {
  // TODO how to implement this with op?
}

export function bufferFromPointer(
  start: BigInt,
  length: number
): SharedArrayBuffer {
  // TODO how to implement this with op?
}
