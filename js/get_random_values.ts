// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

function req(
  typedArray:
    | Int8Array
    | Uint8Array
    | Int16Array
    | Uint16Array
    | Int32Array
    | Uint32Array
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, ArrayBufferView] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.GetRandomValues.createGetRandomValues(builder);
  return [
    builder,
    msg.Any.GetRandomValues,
    inner,
    typedArray as ArrayBufferView
  ];
}

/** Collects cryptographically secure random values. The underlying CSPRNG in
 * use is Rust's `rand::rngs::ThreadRng`.
 *
 *       const arr = new Uint8Array(32);
 *       await Deno.getRandomValues(arr);
 */
export async function getRandomValues(
  typedArray:
    | Int8Array
    | Uint8Array
    | Int16Array
    | Uint16Array
    | Int32Array
    | Uint32Array
): Promise<void> {
  await dispatch.sendAsync(...req(typedArray));
}

/** Synchronously collects cryptographically secure random values. The
 * underlying CSPRNG in use is Rust's `rand::rngs::ThreadRng`.
 *
 *       const arr = new Uint8Array(32);
 *       Deno.getRandomValuesSync(arr);
 */
export function getRandomValuesSync(
  typedArray:
    | Int8Array
    | Uint8Array
    | Int16Array
    | Uint16Array
    | Int32Array
    | Uint32Array
): void {
  dispatch.sendSync(...req(typedArray));
}
