// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { flatbuffers } from "flatbuffers";
import * as util from "./util";

// Re-export some types.
export type Offset = flatbuffers.Offset;
export class ByteBuffer extends flatbuffers.ByteBuffer {}
export interface Builder extends flatbuffers.Builder {
  inUse: boolean;
}

const globalBuilder = new flatbuffers.Builder() as Builder;
globalBuilder.inUse = false;

// This is a wrapper around the real Builder .
// The purpose is to reuse a single ArrayBuffer for every message.
// We can do this because the "control" messages sent to the privileged
// side are guaranteed to be used during the call to libdeno.send().
export function createBuilder(): Builder {
  // tslint:disable-next-line:no-any
  const gb = globalBuilder as any;
  util.assert(!gb.inUse);

  let bb = globalBuilder.dataBuffer();
  // Only create a new backing ArrayBuffer if the previous one had grown very
  // large in capacity. This should only happen rarely.
  if (bb.capacity() > 1024) {
    util.log(`realloc flatbuffer ArrayBuffer because it was ${bb.capacity()}`);
    bb = ByteBuffer.allocate(1024);
  }
  gb.bb = bb;
  // Remaining space in the ByteBuffer.
  gb.space = globalBuilder.dataBuffer().capacity();
  // Minimum alignment encountered so far.
  gb.minalign = 1;
  // The vtable for the current table.
  gb.vtable = null;
  // The amount of fields we're actually using.
  gb.vtable_in_use = 0;
  // Whether we are currently serializing a table.
  gb.isNested = false;
  // Starting offset of the current struct/table.
  gb.object_start = 0;
  // List of offsets of all vtables.
  gb.vtables = [];
  // For the current vector being built.
  gb.vector_num_elems = 0;
  // False omits default values from the serialized data
  gb.force_defaults = false;

  gb.inUse = true;

  return gb as Builder;
}
