// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as hex from "../encoding/hex.ts";
import { Hasher, Message, OutputFormat } from "./hash.ts";
import { SupportedAlgorithm } from "./mod.ts";

const TYPE_ERROR_MSG = "hash: `data` is invalid type";

export class Hash implements Deno.Disposer, Hasher {
  #rid: number;
  #disposed: boolean;

  constructor(algorithm: SupportedAlgorithm) {
    this.#rid = Deno.createHash(algorithm);
    this.#disposed = false;
  }

  /**
   * Update internal state
   * @param data data to update
   */
  update(data: Message): this {
    let msg: Uint8Array;

    if (typeof data === "string") {
      msg = new TextEncoder().encode(data as string);
    } else if (typeof data === "object") {
      if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
        msg = new Uint8Array(data);
      } else {
        throw new Error(TYPE_ERROR_MSG);
      }
    } else {
      throw new Error(TYPE_ERROR_MSG);
    }

    Deno.updateHash(this.#rid, msg);

    return this;
  }

  /** Returns final hash */
  digest(): ArrayBuffer {
    return Deno.digestHash(this.#rid);
  }

  /**
   * Returns hash as a string of given format
   * @param format format of output string (hex or base64). Default is hex
   */
  toString(format: OutputFormat = "hex"): string {
    const hash = this.digest();

    switch (format) {
      case "hex":
        return hex.encodeToString(new Uint8Array(hash));
      case "base64":
        const data = new Uint8Array(hash);
        let dataString = "";
        for (let i = 0; i < data.length; ++i) {
          dataString += String.fromCharCode(data[i]);
        }
        return window.btoa(dataString);
      default:
        throw new Error("hash: invalid format");
    }
  }

  /**
   * Releases all resources
   * `dispose` must be called explicitly in order to avoid resource leak.
   */
  dispose(): void {
    if (!this.#disposed) {
      Deno.close(this.#rid);
      this.#disposed = true;
    }
  }
}
