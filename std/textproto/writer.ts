// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/textproto/writer.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { encode } from "../encoding/utf8.ts";
import { BufWriter } from "../io/bufio.ts";

type WriteState = 0 | 1 | 2 | 3;
const WRITE_STATE_BEGIN = 0;
const WRITE_STATE_BEGIN_LINE = 1;
const WRITE_STATE_CR = 2;
const WRITE_STATE_DATA = 3;

const DOT = encode(".");
const CR = encode("\r");
const LF = encode("\n");
const DOTCRLF = encode(".\r\n");

export interface AsyncCloser {
  close(): Promise<void>;
}

class DotWriter implements Deno.Writer, AsyncCloser {
  #state: WriteState = WRITE_STATE_BEGIN;

  constructor(readonly w: TextProtoWriter) {}

  async write(p: Uint8Array): Promise<number> {
    const bufWriter = this.w.bufWriter;
    let n = 0;
    while (n < p.length) {
      const c = p[n];
      switch (this.#state) {
        case WRITE_STATE_BEGIN:
        case WRITE_STATE_BEGIN_LINE:
          this.#state = WRITE_STATE_DATA;
          if (c === charCode(".")) {
            await bufWriter.write(DOT);
          }
          // FALLTHROUGH
        case WRITE_STATE_DATA:
          if (c === charCode("\r")) {
            this.#state = WRITE_STATE_CR;
          }
          if (c === charCode("\n")) {
            await bufWriter.write(CR);
            this.#state = WRITE_STATE_BEGIN_LINE;
          }
          break;
        case WRITE_STATE_CR:
          this.#state = WRITE_STATE_DATA;
          if (c === charCode("\n")) {
            this.#state = WRITE_STATE_BEGIN_LINE;
          }
          break;
      }
      await bufWriter.write(p.subarray(n, n + 1));
      n++;
    }
    return n;
  }

  async close(): Promise<void> {
    if (this.w._dotWriter === this) {
      this.w._dotWriter = null;
    }
    const bw = this.w.bufWriter;
    switch (this.#state) {
      default:
        await bw.write(CR);
        // FALLTHROUGH
      case WRITE_STATE_CR:
        await bw.write(LF);
        // FALLTHROUGH
      case WRITE_STATE_BEGIN_LINE:
        await bw.write(DOTCRLF);
        break;
    }
    return bw.flush();
  }
}

/**
 * @description A `TextProtoWriter` implements convenience methods for writing
 *   requests or responses to a text protocol network connection.
 */
export class TextProtoWriter {
  /**
   * @private
   */
  _dotWriter: DotWriter | null = null;

  constructor(
    readonly bufWriter: BufWriter,
  ) {
  }

  /**
   * @description `printLine` writes the output followed by \r\n.
   */
  async printLine(line: string): Promise<void> {
    this.closeDot();
    await this.bufWriter.write(encode(line));
    await this.bufWriter.write(CR);
    await this.bufWriter.write(LF);
    return this.bufWriter.flush();
  }

  /**
   * @description `dotWriter` returns a `Writer` that can be used to write a dot-encoding to `bufWriter`.
   *   It takes care of inserting leading dots when necessary,
   *   translating line-ending \n into \r\n, and adding the final .\r\n line
   *   when the `dotWriter` is closed. The caller should close the
   *   `dotWriter` before the next call to a method on w.
   * 
   *   See the documentation for Reader's `dotReader` method for details about dot-encoding.
   */
  dotWriter(): Deno.Writer & AsyncCloser {
    this.closeDot();
    const dotWriter = new DotWriter(this);
    this._dotWriter = dotWriter;
    return dotWriter;
  }

  private closeDot(): void {
    if (this._dotWriter !== null) {
      this._dotWriter.close();
    }
  }
}

function charCode(c: string): number {
  return c.charCodeAt(0);
}
