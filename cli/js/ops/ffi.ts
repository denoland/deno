// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function lookup(rid: number, symbol: string): MemoryAddr {
  // TODO return MemoryAddr
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

/** An opaque pointer */
export class MemoryAddr {
  /** Return the memory address to the start of the `buffer`.
   * Requires --allow-ffi. */
  static fromBuffer(buffer: SharedArrayBuffer): MemoryAddr {
    // TODO implement this
  }

  /** Construct a buffer with access to a fixed chunk of memory.
   * Requires --allow-ffi. */
  toBuffer(length: number): SharedArrayBuffer {
    // TODO implement this
  }

  static fromHex(addr: string): MemoryAddr {
    // TODO implement this
  }
  toHex(): string {
    // TODO implement this
  }
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
