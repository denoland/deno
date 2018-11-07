import * as deno from "deno";

const DEFAULT_BUF_SIZE = 4096;
const MIN_BUF_SIZE = 16;
const MAX_CONSECUTIVE_EMPTY_READS = 100;

export class Reader implements deno.Reader {
  private buf: Uint8Array;
  private rd: deno.Reader; // Reader provided by caller.
  private r = 0; // buf read position.
  private w = 0; // buf write position.
  private lastByte: number;
  private lastCharSize: number;

  constructor(rd: deno.Reader, size = DEFAULT_BUF_SIZE) {
    if (size < MIN_BUF_SIZE) {
      size = MIN_BUF_SIZE;
    }
    this._reset(new Uint8Array(size), rd)
  }

  /** Returns the size of the underlying buffer in bytes. */
  get byteLength(): number {
    return this.buf.byteLength;
  }

  // Reads a new chunk into the buffer.
  // Returns true if EOF, false on successful read.
  async _fill(): Promise<boolean> {
    // Slide existing data to beginning.
    if (this.r > 0) {
      this.buf.copyWithin(0, this.r, this.w);
      this.w -= this.r;
      this.r = 0;
    }

    if (this.w >= this.buf.byteLength) {
      throw Error("bufio: tried to fill full buffer");
    }

    // Read new data: try a limited number of times.
    for (let i = MAX_CONSECUTIVE_EMPTY_READS; i > 0; i--) {
      const { nread, eof } = await this.rd.read(this.buf.subarray(this.w));
      if (nread < 0) {
        throw Error("negative read");
      }
      this.w += nread;
      if (eof) {
        return true;
      }
      if (nread > 0) {
        return false;
      }
    }
    throw Error("No Progress");
  }

  /** Discards any buffered data, resets all state, and switches
   * the buffered reader to read from r.
   */
  reset(r: deno.Reader): void {
    this._reset(this.buf, r);
  }

  private _reset(buf: Uint8Array, rd: deno.Reader): void {
    this.buf = buf;
    this.rd = rd;
    this.lastByte = -1;
    this.lastCharSize = -1;
  }

  async read(p: ArrayBufferView): Promise<deno.ReadResult> {
    throw Error("not implemented");
    return { nread: 0, eof: false };
  }

  /** Returns the next byte [0, 255] or -1 if EOF. */
  async readByte(): Promise<number> {
    while (this.r === this.w) {
      const eof = await this._fill(); // buffer is empty.
      if (eof) {
        return -1;
      }
    }
    const c = this.buf[this.r];
    this.r++;
    this.lastByte = c;
    return c;
  }
}


