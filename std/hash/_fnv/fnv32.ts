// Ported from Go:
// https://github.com/golang/go/tree/go1.13.10/src/hash/fnv/fnv.go
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { mul32 } from "./util.ts";

const offset32 = 2166136261;
const prime32 = 16777619;

abstract class Fnv32Base<T> {
  #state: number;

  constructor() {
    this.#state = offset32;
  }

  protected _updateState(newState: number): void {
    this.#state = newState;
  }

  reset(): void {
    this.#state = offset32;
  }

  abstract write(data: Uint8Array): T;

  size(): number {
    return 4;
  }

  blockSize(): number {
    return 1;
  }

  sum32(): number {
    return this.#state;
  }

  sum(): Uint8Array {
    return Uint8Array.from([
      (this.#state >> 24) & 0xff,
      (this.#state >> 16) & 0xff,
      (this.#state >> 8) & 0xff,
      this.#state & 0xff,
    ]);
  }
}

/** Fnv32 hash */
export class Fnv32 extends Fnv32Base<Fnv32> {
  write(data: Uint8Array): Fnv32 {
    let hash = this.sum32();

    data.forEach((c) => {
      hash = mul32(hash, prime32);
      hash ^= c;
    });

    this._updateState(hash);
    return this;
  }
}

/** Fnv32a hash */
export class Fnv32a extends Fnv32Base<Fnv32a> {
  write(data: Uint8Array): Fnv32a {
    let hash = this.sum32();

    data.forEach((c) => {
      hash ^= c;
      hash = mul32(hash, prime32);
    });

    this._updateState(hash);
    return this;
  }
}
