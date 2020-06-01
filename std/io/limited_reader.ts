// Based on https://github.com/golang/go/blob/0452f9460f50f0f0aba18df43dc2b31906fb66cc/src/io/io.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

/**
 * A `LimitedReader` reads from `reader` but limits the amount of data returned to just `limit` bytes.
 * Each call to `read` updates `limit` to reflect the new amount remaining.
 * `read` returns `null` when `limit` <= `0` or
 * when the underlying `reader` returns `null`.
 */
export class LimitedReader implements Deno.Reader {
  constructor(public reader: Deno.Reader, public limit: number) {}

  async read(p: Uint8Array): Promise<number | null> {
    if (this.limit <= 0) {
      return null;
    }

    if (p.length > this.limit) {
      p = p.subarray(0, this.limit);
    }
    const n = await this.reader.read(p);
    if (n == null) {
      return null;
    }

    this.limit -= n;
    return n;
  }
}
