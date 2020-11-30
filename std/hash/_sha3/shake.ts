// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { keccakf } from "./keccakf.ts";
import { Sponge } from "./sponge.ts";

/** Shake128 hash */
export class Shake128 extends Sponge {
  /**
   * Instantiates a new Shake128 hash
   * @param bitsize length of hash in bits
   */
  constructor(bitsize: number) {
    if (bitsize < 8) {
      throw new Error("shake128: `bitsize` too small");
    }

    if (bitsize % 8 !== 0) {
      throw new Error("shake128: `bitsize` must be multiple of 8");
    }

    super({
      bitsize: bitsize,
      rate: 168,
      dsbyte: 0x1f,
      permutator: keccakf,
    });
  }
}

/**
 * Instantiates a new Shake256 hash
 * @param bitsize length of hash in bits
 */
export class Shake256 extends Sponge {
  constructor(bitsize: number) {
    if (bitsize < 8) {
      throw new Error("shake256: `bitsize` too small");
    }

    if (bitsize % 8 !== 0) {
      throw new Error("shake256: `bitsize` must be multiple of 8");
    }

    super({
      bitsize: bitsize,
      rate: 136,
      dsbyte: 0x1f,
      permutator: keccakf,
    });
  }
}
