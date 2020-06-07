// Ported to Deno from
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
type ReaderSync = Deno.ReaderSync;

/** OneByteReader returns a ReaderSync that implements
 * each non-empty Read by reading one byte from r.
 */
export class OneByteReader implements ReaderSync {
  constructor(readonly r: ReaderSync) {}

  readSync(p: Uint8Array): number | null {
    if (p.byteLength === 0) {
      return 0;
    }
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    return this.r.readSync(p.subarray(0, 1));
  }
}

/** HalfReader returns a ReaderSync that implements Read
 * by reading half as many requested bytes from r.
 */
export class HalfReader implements ReaderSync {
  constructor(readonly r: ReaderSync) {}

  readSync(p: Uint8Array): number | null {
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    const half = Math.floor((p.byteLength + 1) / 2);
    return this.r.readSync(p.subarray(0, half));
  }
}

/** TimeOutReader returns `Deno.errors.TimedOut` on the second read
 * with no data. Subsequent calls to read succeed.
 */
export class TimeOutReader implements ReaderSync {
  count = 0;
  constructor(readonly r: ReaderSync) {}

  readSync(p: Uint8Array): number | null {
    this.count++;
    if (this.count === 2) {
      throw new Deno.errors.TimedOut();
    }
    return this.r.readSync(p);
  }
}
