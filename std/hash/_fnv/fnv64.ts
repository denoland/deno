// Ported from Go:
// https://github.com/golang/go/tree/go1.13.10/src/hash/fnv/fnv.go
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { mul64 } from "./util.ts";

const offset64Lo = 2216829733;
const offset64Hi = 3421674724;
const prime64Lo = 435;
const prime64Hi = 256;

abstract class Fnv64Base<T> {
  #stateHi: number;
  #stateLo: number;

  constructor() {
    this.#stateHi = offset64Hi;
    this.#stateLo = offset64Lo;
  }

  protected _updateState([newStateHi, newStateLo]: [number, number]): void {
    this.#stateHi = newStateHi;
    this.#stateLo = newStateLo;
  }

  reset(): void {
    this.#stateHi = offset64Hi;
    this.#stateLo = offset64Lo;
  }

  abstract write(data: Uint8Array): T;

  size(): number {
    return 8;
  }

  blockSize(): number {
    return 1;
  }

  sum64(): [number, number] {
    return [this.#stateHi, this.#stateLo];
  }

  sum(): Uint8Array {
    return Uint8Array.from([
      (this.#stateHi >> 24) & 0xff,
      (this.#stateHi >> 16) & 0xff,
      (this.#stateHi >> 8) & 0xff,
      this.#stateHi & 0xff,
      (this.#stateLo >> 24) & 0xff,
      (this.#stateLo >> 16) & 0xff,
      (this.#stateLo >> 8) & 0xff,
      this.#stateLo & 0xff,
    ]);
  }
}

/** Fnv64 hash */
export class Fnv64 extends Fnv64Base<Fnv64> {
  write(data: Uint8Array): Fnv64 {
    let [hashHi, hashLo] = this.sum64();

    data.forEach((c) => {
      [hashHi, hashLo] = mul64([hashHi, hashLo], [prime64Hi, prime64Lo]);
      hashLo ^= c;
    });

    this._updateState([hashHi, hashLo]);
    return this;
  }
}

/** Fnv64a hash */
export class Fnv64a extends Fnv64Base<Fnv64a> {
  write(data: Uint8Array): Fnv64 {
    let [hashHi, hashLo] = this.sum64();

    data.forEach((c) => {
      hashLo ^= c;
      [hashHi, hashLo] = mul64([hashHi, hashLo], [prime64Hi, prime64Lo]);
    });

    this._updateState([hashHi, hashLo]);
    return this;
  }
}
