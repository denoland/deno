// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import init, {
  source,
  HmacSha256Hash,
  HmacSha512Hash,
} from "./wasm.js";

import type { HmacHasher, Message } from "../hmac_hasher.ts";

await init(source);

type HmacHashMethods = HmacSha256Hash | HmacSha512Hash;

export class HmacHash implements HmacHasher {
  #inner: HmacHashMethods;
  #digested: boolean;

  constructor(algorithm: string,key: string) {
    switch(algorithm) {
      case "sha256": {
        this.#inner = new HmacSha256Hash(key);
        break;
      }
      case "sha512": {
        this.#inner = new HmacSha512Hash(key);
        break;
      }
      default: {
        throw new Error("hmacHash: algorithm unsupported");
      }
    }
    this.#digested = false;
  }

  /**
   * Update internal state
   * @param data data to update
   */
  update(data: Message): this {

    this.#inner.update(data);
    return this;
  }

  /** Returns final hash */
  digest(): ArrayBuffer {
    console.log("digest",this.#inner,this.#digested);
    if (this.#digested) throw new Error("hash: already digested");

    this.#digested = true;
    return this.#inner.digest();
  }
}
