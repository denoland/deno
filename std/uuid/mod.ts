// Based on https://github.com/kelektiv/node-uuid
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as v1 from "./v1.ts";
import * as v4 from "./v4.ts";
import * as v5 from "./v5.ts";

export const NIL_UUID = "00000000-0000-0000-0000-000000000000";

export function isNil(val: string): boolean {
  return val === NIL_UUID;
}

const NOT_IMPLEMENTED = {
  generate(): never {
    throw new Error("Not implemented");
  },
  validate(): never {
    throw new Error("Not implemented");
  },
};

// TODO Implement
export const v3 = NOT_IMPLEMENTED;

export { v1, v4, v5 };
