// Based on https://github.com/kelektiv/node-uuid
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

export const NIL_UUID = "00000000-0000-0000-0000-000000000000";

export function isNil(val: string): boolean {
  return val === NIL_UUID;
}

const NOT_IMPLEMENTED = (): void => {
  throw new Error("Not implemented");
};

// TODO Implement
export const v1 = NOT_IMPLEMENTED;
// TODO Implement
export const v3 = NOT_IMPLEMENTED;

import _v4 from "./v4.ts";
export const v4 = _v4;

// TODO Implement
export const v5 = NOT_IMPLEMENTED;

export default v4;
export * from "./v4.ts";
