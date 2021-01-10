// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import * as hex from "../../encoding/hex.ts";

type SpongePermutator = (data: Uint8Array) => void;

/** Sponge construction option */
export interface SpongeOption {
  bitsize: number;
  rate: number;
  dsbyte: number;
  permutator: SpongePermutator;
}

export type Message = string | ArrayBuffer;

const STATE_SIZE = 200;
const TYPE_ERROR_MSG = "sha3: `data` is invalid type";

/** Sponge construction */
export class Sponge {
  #option: SpongeOption;
  #state: Uint8Array;
  #rp: number;
  #absorbing: boolean;

  constructor(option: SpongeOption) {
    this.#option = option;
    this.#state = new Uint8Array(STATE_SIZE);
    this.#rp = 0;
    this.#absorbing = true;
  }

  /** Applies padding to internal state */
  private pad(): void {
    this.#state[this.#rp] ^= this.#option.dsbyte;
    this.#state[this.#option.rate - 1] ^= 0x80;
  }

  /** Squeezes internal state */
  protected squeeze(length: number): Uint8Array {
    if (length < 0) {
      throw new Error("sha3: length cannot be negative");
    }

    this.pad();

    const hash = new Uint8Array(length);
    let pos = 0;
    while (length > 0) {
      const r = length > this.#option.rate ? this.#option.rate : length;
      this.#option.permutator(this.#state);
      hash.set(this.#state.slice(0, r), pos);
      length -= r;
      pos += r;
    }

    this.#absorbing = false;
    return hash;
  }

  /** Updates internal state by absorbing */
  update(data: Message): this {
    if (!this.#absorbing) {
      throw new Error("sha3: cannot update already finalized hash");
    }

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

    let rp = this.#rp;

    for (let i = 0; i < msg.length; ++i) {
      this.#state[rp++] ^= msg[i];
      if (rp >= this.#option.rate) {
        this.#option.permutator(this.#state);
        rp = 0;
      }
    }

    this.#rp = rp;
    return this;
  }

  /** Returns the hash in ArrayBuffer */
  digest(): ArrayBuffer {
    return this.squeeze(this.#option.bitsize >> 3);
  }

  /** Returns the hash in given format */
  toString(format: "hex" = "hex"): string {
    const rawOutput = this.squeeze(this.#option.bitsize >> 3);
    switch (format) {
      case "hex":
        return hex.encodeToString(rawOutput);
      default:
        throw new Error("sha3: invalid output format");
    }
  }
}
