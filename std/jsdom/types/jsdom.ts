// TODO: Use proper types from https://github.com/DefinitelyTyped/DefinitelyTyped/blob/master/types/jsdom/index.d.ts.

import { dom } from "./dom.ts";

export namespace jsdom {
  export interface JSDOM {
    // eslint-disable-next-line @typescript-eslint/no-misused-new
    new (
      html: string | BinaryData,
      options?: { runScripts?: "dangerously" }
    ): JSDOM;
    window: dom.Window;
    fragment: (html: string) => dom.DocumentFragment;
    serialize(): string;
  }

  export type BinaryData = ArrayBuffer | DataView | TypedArray;

  export type TypedArray =
    | Int8Array
    | Uint8Array
    | Uint8ClampedArray
    | Int16Array
    | Uint16Array
    | Int32Array
    | Uint32Array
    | Float32Array
    | Float64Array;
}
