// Copyright Node.js contributors. All rights reserved. MIT License.
import { Encodings } from "../_utils.ts";
import Duplex from "./duplex.ts";
import type { DuplexOptions } from "./duplex.ts";
import type { writeV } from "./writable_internal.ts";
import { ERR_METHOD_NOT_IMPLEMENTED } from "../_errors.ts";

const kCallback = Symbol("kCallback");

type TransformFlush = (
  this: Transform,
  // deno-lint-ignore no-explicit-any
  callback: (error?: Error | null, data?: any) => void,
) => void;

export interface TransformOptions extends DuplexOptions {
  read?(this: Transform, size: number): void;
  write?(
    this: Transform,
    // deno-lint-ignore no-explicit-any
    chunk: any,
    encoding: Encodings,
    callback: (error?: Error | null) => void,
  ): void;
  writev?: writeV;
  final?(this: Transform, callback: (error?: Error | null) => void): void;
  destroy?(
    this: Transform,
    error: Error | null,
    callback: (error: Error | null) => void,
  ): void;
  transform?(
    this: Transform,
    // deno-lint-ignore no-explicit-any
    chunk: any,
    encoding: Encodings,
    // deno-lint-ignore no-explicit-any
    callback: (error?: Error | null, data?: any) => void,
  ): void;
  flush?: TransformFlush;
}

export default class Transform extends Duplex {
  [kCallback]: null | ((error?: Error | null) => void);
  _flush?: TransformFlush;

  constructor(options?: TransformOptions) {
    super(options);
    this._readableState.sync = false;

    this[kCallback] = null;

    if (options) {
      if (typeof options.transform === "function") {
        this._transform = options.transform;
      }

      if (typeof options.flush === "function") {
        this._flush = options.flush;
      }
    }

    this.on("prefinish", function (this: Transform) {
      if (typeof this._flush === "function" && !this.destroyed) {
        this._flush((er, data) => {
          if (er) {
            this.destroy(er);
            return;
          }

          if (data != null) {
            this.push(data);
          }
          this.push(null);
        });
      } else {
        this.push(null);
      }
    });
  }

  _read = () => {
    if (this[kCallback]) {
      const callback = this[kCallback] as (error?: Error | null) => void;
      this[kCallback] = null;
      callback();
    }
  };

  _transform(
    // deno-lint-ignore no-explicit-any
    _chunk: any,
    _encoding: string,
    // deno-lint-ignore no-explicit-any
    _callback: (error?: Error | null, data?: any) => void,
  ) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("_transform()");
  }

  _write = (
    // deno-lint-ignore no-explicit-any
    chunk: any,
    encoding: string,
    callback: (error?: Error | null) => void,
  ) => {
    const rState = this._readableState;
    const wState = this._writableState;
    const length = rState.length;

    this._transform(chunk, encoding, (err, val) => {
      if (err) {
        callback(err);
        return;
      }

      if (val != null) {
        this.push(val);
      }

      if (
        wState.ended || // Backwards compat.
        length === rState.length || // Backwards compat.
        rState.length < rState.highWaterMark ||
        rState.length === 0
      ) {
        callback();
      } else {
        this[kCallback] = callback;
      }
    });
  };
}
