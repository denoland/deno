import { debug } from "./main";
import { TypedArray } from "./types";

// Internal logging for deno. Use the "debug" variable above to control
// output.
// tslint:disable-next-line:no-any
export function log(...args: any[]): void {
  if (debug) {
    console.log(...args);
  }
}

export function assert(cond: boolean, msg = "") {
  if (!cond) {
    throw Error("Assert fail. " + msg);
  }
}

export function typedArrayToArrayBuffer(ta: TypedArray): ArrayBuffer {
  const ab = ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
  return ab as ArrayBuffer;
}
