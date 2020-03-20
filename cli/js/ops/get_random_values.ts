// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";
import { assert } from "../util.ts";

export function getRandomValues<
  T extends
    | Int8Array
    | Uint8Array
    | Uint8ClampedArray
    | Int16Array
    | Uint16Array
    | Int32Array
    | Uint32Array
>(typedArray: T): T {
  assert(typedArray !== null, "Input must not be null");
  assert(typedArray.length <= 65536, "Input must not be longer than 65536");
  const ui8 = new Uint8Array(
    typedArray.buffer,
    typedArray.byteOffset,
    typedArray.byteLength
  );
  sendSync("op_get_random_values", {}, ui8);
  return typedArray;
}
