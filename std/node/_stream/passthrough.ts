// Copyright Node.js contributors. All rights reserved. MIT License.
import type { Encodings } from "../_utils.ts";
import Transform from "./transform.ts";
import type { TransformOptions } from "./transform.ts";

export default class PassThrough extends Transform {
  constructor(options?: TransformOptions) {
    super(options);
  }

  _transform(
    // deno-lint-ignore no-explicit-any
    chunk: any,
    _encoding: Encodings,
    // deno-lint-ignore no-explicit-any
    cb: (error?: Error | null, data?: any) => void,
  ) {
    cb(null, chunk);
  }
}
