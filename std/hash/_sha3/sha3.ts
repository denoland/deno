// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { Sponge } from "./sponge.ts";
import { keccakf } from "./keccakf.ts";

/** Sha3-224 hash */
// deno-lint-ignore camelcase
export class Sha3_224 extends Sponge {
  constructor() {
    super({
      bitsize: 224,
      rate: 144,
      dsbyte: 6,
      permutator: keccakf,
    });
  }
}

/** Sha3-256 hash */
// deno-lint-ignore camelcase
export class Sha3_256 extends Sponge {
  constructor() {
    super({
      bitsize: 256,
      rate: 136,
      dsbyte: 6,
      permutator: keccakf,
    });
  }
}

/** Sha3-384 hash */
// deno-lint-ignore camelcase
export class Sha3_384 extends Sponge {
  constructor() {
    super({
      bitsize: 384,
      rate: 104,
      dsbyte: 6,
      permutator: keccakf,
    });
  }
}

/** Sha3-512 hash */
// deno-lint-ignore camelcase
export class Sha3_512 extends Sponge {
  constructor() {
    super({
      bitsize: 512,
      rate: 72,
      dsbyte: 6,
      permutator: keccakf,
    });
  }
}
