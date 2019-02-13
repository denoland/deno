// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { flatbuffers } from "flatbuffers";
import { libdeno } from "./libdeno";

// Re-export some types.
export type Offset = flatbuffers.Offset;
export class ByteBuffer extends flatbuffers.ByteBuffer {}
export interface Builder extends flatbuffers.Builder {
}


// const oldAllocate = flatbuffers.ByteBuffer.allocate;
flatbuffers.ByteBuffer.allocate = (size: number): flatbuffers.ByteBuffer => {
  // throw Error("ba");
  // return oldAllocate(size);
 
  const { byteOffset, byteLength } = libdeno.tx.beginSend(size);
  console.log("ByteBuffer.allocate slice: ", byteOffset, byteLength);
  const bytes = libdeno.tx.u8.subarray(byteOffset, byteOffset + byteLength);
  const bb = new flatbuffers.ByteBuffer(bytes);

  // const bb = new flatbuffers.ByteBuffer(libdeno.tx.u8);
  // bb.setPosition(byteOffset);
  return bb;
};

export function createBuilder(): Builder {
  let builder = new flatbuffers.Builder();
  return builder;
}
