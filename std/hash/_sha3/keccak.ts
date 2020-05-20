// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Sponge } from "./sponge.ts";
import { keccakf } from "./keccakf.ts";

/** Keccak-224 hash */
export class Keccak224 extends Sponge {
  constructor() {
    super({
      bitsize: 224,
      rate: 144,
      dsbyte: 1,
      permutator: keccakf,
    });
  }
}

/** Keccak-256 hash */
export class Keccak256 extends Sponge {
  constructor() {
    super({
      bitsize: 256,
      rate: 136,
      dsbyte: 1,
      permutator: keccakf,
    });
  }
}

/** Keccak-384 hash */
export class Keccak384 extends Sponge {
  constructor() {
    super({
      bitsize: 384,
      rate: 104,
      dsbyte: 1,
      permutator: keccakf,
    });
  }
}

/** Keccak-512 hash */
export class Keccak512 extends Sponge {
  constructor() {
    super({
      bitsize: 512,
      rate: 72,
      dsbyte: 1,
      permutator: keccakf,
    });
  }
}
