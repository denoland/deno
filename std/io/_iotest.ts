// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.// Ported to Deno from
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
type Reader = Deno.Reader;

/** OneByteReader returns a Reader that implements
 * each non-empty Read by reading one byte from r.
 */
export class OneByteReader implements Reader {
  constructor(readonly r: Reader) {}

  read(p: Uint8Array): Promise<number | null> {
    if (p.byteLength === 0) {
      return Promise.resolve(0);
    }
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    return Promise.resolve(this.r.read(p.subarray(0, 1)));
  }
}

/** HalfReader returns a Reader that implements Read
 * by reading half as many requested bytes from r.
 */
export class HalfReader implements Reader {
  constructor(readonly r: Reader) {}

  read(p: Uint8Array): Promise<number | null> {
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    const half = Math.floor((p.byteLength + 1) / 2);
    return Promise.resolve(this.r.read(p.subarray(0, half)));
  }
}

/** TimeoutReader returns `Deno.errors.TimedOut` on the second read
 * with no data. Subsequent calls to read succeed.
 */
export class TimeoutReader implements Reader {
  count = 0;
  constructor(readonly r: Reader) {}

  read(p: Uint8Array): Promise<number | null> {
    this.count++;
    if (this.count === 2) {
      throw new Deno.errors.TimedOut();
    }
    return Promise.resolve(this.r.read(p));
  }
}
