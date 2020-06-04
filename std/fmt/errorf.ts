// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// This implementation is inspired by Golang but does not port
// implementation code.
import { Printf } from "./printf.ts";

// Errorf formats according to a format specifier and returns the string as a
// value that satisfies error.
export function errorf(format: string, ...args: unknown[]): Error {
  const p = new Printf(format, ...args);
  p.doPrintf();
  const s = p.buf.toString();
  const err = new Error(s);
  return err;
}
