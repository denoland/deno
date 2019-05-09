// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";

function req(
  typedArray: ArrayBufferView
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, ArrayBufferView] {
  assert(typedArray != null);
  const builder = flatbuffers.createBuilder();
  const inner = msg.GetRandomValues.createGetRandomValues(builder);
  return [builder, msg.Any.GetRandomValues, inner, typedArray];
}

/** Collects cryptographically secure random values. Throws if given anything
 * but an ArrayBufferView. The underlying CSPRNG in use is Rust's
 * `rand::rngs::ThreadRng`.
 *
 *       const arr = new Uint8Array(32);
 *       await Deno.getRandomValues(arr);
 */
export async function getRandomValues(typedArray: ArrayBufferView): Promise<void> {
  await dispatch.sendAsync(...req(typedArray));
}

/** Synchronously collects cryptographically secure random values. Throws if
 * given anything but an ArrayBufferView. The underlying CSPRNG in use is Rust's
 * `rand::rngs::ThreadRng`.
 *
 *       const arr = new Uint8Array(32);
 *       Deno.getRandomValuesSync(arr);
 */
export function getRandomValuesSync(typedArray: ArrayBufferView): void {
  dispatch.sendSync(...req(typedArray));
}
