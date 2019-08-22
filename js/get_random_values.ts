// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, msg, flatbuffers } from "./dispatch_flatbuffers";
import { assert } from "./util";

function req(
  typedArray: ArrayBufferView
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, ArrayBufferView] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.GetRandomValues.createGetRandomValues(builder);
  return [builder, msg.Any.GetRandomValues, inner, typedArray];
}

/** Synchronously collects cryptographically secure random values. The
 * underlying CSPRNG in use is Rust's `rand::rngs::ThreadRng`.
 *
 *       const arr = new Uint8Array(32);
 *       crypto.getRandomValues(arr);
 */
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
  sendSync(...req(typedArray as ArrayBufferView));
  return typedArray;
}
