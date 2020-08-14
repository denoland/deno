// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import init, {
  source,
  create_hash as createHash,
  update_hash as updateHash,
  digest_hash as digestHash,
  DenoHash,
} from "./wasm.js";

import * as hex from "../../encoding/hex.ts";
import * as base64 from "../../encoding/base64.ts";
import type { Hasher, Message, OutputFormat } from "../hasher.ts";

await init(source);

const TYPE_ERROR_MSG = "hash: `data` is invalid type";

export class Hash implements Hasher {
  #hash: DenoHash;
  #digested: boolean;

  constructor(algorithm: string) {
    this.#hash = createHash(algorithm);
    this.#digested = false;
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

    updateHash(this.#hash, msg);

    return this;
  }

  /** Returns final hash */
  digest(): ArrayBuffer {
    if (this.#digested) throw new Error("hash: already digested");

    this.#digested = true;
    return digestHash(this.#hash);
  }

  /**
   * Returns hash as a string of given format
   * @param format format of output string (hex or base64). Default is hex
   */
  toString(format: OutputFormat = "hex"): string {
    const finalized = new Uint8Array(this.digest());

    switch (format) {
      case "hex":
        return hex.encodeToString(finalized);
      case "base64":
        return base64.encode(finalized);
      default:
        throw new Error("hash: invalid format");
    }
  }
}
