// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { lookup as defaultLookup } from "node:dns";
import {
  isInt32,
  validateFunction,
} from "ext:deno_node/internal/validators.mjs";
import type { ErrnoException } from "ext:deno_node/internal/errors.ts";
import { ERR_SOCKET_BAD_TYPE } from "ext:deno_node/internal/errors.ts";
import { UDP } from "ext:deno_node/internal_binding/udp_wrap.ts";
import { guessHandleType } from "ext:deno_node/internal_binding/util.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { primordials } from "ext:core/mod.js";
const { FunctionPrototypeBind, MapPrototypeGet, Symbol } = primordials;

export type SocketType = "udp4" | "udp6";

export const kStateSymbol: unique symbol = Symbol("kStateSymbol");

function lookup4(
  lookup: typeof defaultLookup,
  address: string,
  callback: (
    err: ErrnoException | null,
    address: string,
    family: number,
  ) => void,
) {
  return lookup(address || "127.0.0.1", 4, callback);
}

function lookup6(
  lookup: typeof defaultLookup,
  address: string,
  callback: (
    err: ErrnoException | null,
    address: string,
    family: number,
  ) => void,
) {
  return lookup(address || "::1", 6, callback);
}

export function newHandle(
  type: SocketType,
  lookup?: typeof defaultLookup,
): UDP {
  if (lookup === undefined) {
    lookup = defaultLookup;
  } else {
    validateFunction(lookup, "lookup");
  }

  if (type === "udp4") {
    const handle = new UDP();

    handle.lookup = FunctionPrototypeBind(lookup4, handle, lookup);

    return handle;
  }

  if (type === "udp6") {
    const handle = new UDP();

    handle.lookup = FunctionPrototypeBind(lookup6, handle, lookup);
    handle.bind = handle.bind6;
    handle.connect = handle.connect6;
    handle.send = handle.send6;

    return handle;
  }

  throw new ERR_SOCKET_BAD_TYPE();
}

export function _createSocketHandle(
  address: string,
  port: number,
  addressType: SocketType,
  fd: number,
  flags: number,
) {
  const handle = newHandle(addressType);
  let err;

  if (isInt32(fd) && fd > 0) {
    const type = guessHandleType(fd);

    if (type !== "UDP") {
      err = MapPrototypeGet(codeMap, "EINVAL")!;
    } else {
      err = handle.open(fd);
    }
  } else if (port || address) {
    // deno-lint-ignore prefer-primordials
    err = handle.bind(address, port || 0, flags);
  }

  if (err) {
    handle.close();

    return err;
  }

  return handle;
}

export default {
  kStateSymbol,
  newHandle,
  _createSocketHandle,
};
