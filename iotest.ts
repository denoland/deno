// Ported to Deno from
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { Reader, ReadResult } from "deno";

/** OneByteReader returns a Reader that implements
 * each non-empty Read by reading one byte from r.
 */
export class OneByteReader implements Reader {
  constructor(readonly r: Reader) {}

  async read(p: Uint8Array): Promise<ReadResult> {
    if (p.byteLength === 0) {
      return { nread: 0, eof: false };
    }
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    return this.r.read(p.subarray(0, 1));
  }
}

/** HalfReader returns a Reader that implements Read
 * by reading half as many requested bytes from r.
 */
export class HalfReader implements Reader {
  constructor(readonly r: Reader) {}

  async read(p: Uint8Array): Promise<ReadResult> {
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    const half = Math.floor((p.byteLength + 1) / 2);
    return this.r.read(p.subarray(0, half));
  }
}

export class ErrTimeout extends Error {
  constructor() {
    super("timeout");
    this.name = "ErrTimeout";
  }
}

/** TimeoutReader returns ErrTimeout on the second read
 * with no data. Subsequent calls to read succeed.
 */
export class TimeoutReader implements Reader {
  count = 0;
  constructor(readonly r: Reader) {}

  async read(p: Uint8Array): Promise<ReadResult> {
    this.count++;
    if (this.count === 2) {
      throw new ErrTimeout();
    }
    return this.r.read(p);
  }
}
