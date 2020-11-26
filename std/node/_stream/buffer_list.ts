// Copyright Node.js contributors. All rights reserved. MIT License.
import { Buffer } from "../buffer.ts";

type BufferListItem = {
  data: Buffer | string | Uint8Array;
  next: BufferListItem | null;
};

export default class BufferList {
  head: BufferListItem | null = null;
  tail: BufferListItem | null = null;
  length: number;

  constructor() {
    this.head = null;
    this.tail = null;
    this.length = 0;
  }

  push(v: Buffer | string | Uint8Array) {
    const entry = { data: v, next: null };
    if (this.length > 0) {
      (this.tail as BufferListItem).next = entry;
    } else {
      this.head = entry;
    }
    this.tail = entry;
    ++this.length;
  }

  unshift(v: Buffer | string | Uint8Array) {
    const entry = { data: v, next: this.head };
    if (this.length === 0) {
      this.tail = entry;
    }
    this.head = entry;
    ++this.length;
  }

  shift() {
    if (this.length === 0) {
      return;
    }
    const ret = (this.head as BufferListItem).data;
    if (this.length === 1) {
      this.head = this.tail = null;
    } else {
      this.head = (this.head as BufferListItem).next;
    }
    --this.length;
    return ret;
  }

  clear() {
    this.head = this.tail = null;
    this.length = 0;
  }

  join(s: string) {
    if (this.length === 0) {
      return "";
    }
    let p: BufferListItem | null = (this.head as BufferListItem);
    let ret = "" + p.data;
    p = p.next;
    while (p) {
      ret += s + p.data;
      p = p.next;
    }
    return ret;
  }

  concat(n: number) {
    if (this.length === 0) {
      return Buffer.alloc(0);
    }
    const ret = Buffer.allocUnsafe(n >>> 0);
    let p = this.head;
    let i = 0;
    while (p) {
      ret.set(p.data as Buffer, i);
      i += p.data.length;
      p = p.next;
    }
    return ret;
  }

  // Consumes a specified amount of bytes or characters from the buffered data.
  consume(n: number, hasStrings: boolean) {
    const data = (this.head as BufferListItem).data;
    if (n < data.length) {
      // `slice` is the same for buffers and strings.
      const slice = data.slice(0, n);
      (this.head as BufferListItem).data = data.slice(n);
      return slice;
    }
    if (n === data.length) {
      // First chunk is a perfect match.
      return this.shift();
    }
    // Result spans more than one buffer.
    return hasStrings ? this._getString(n) : this._getBuffer(n);
  }

  first() {
    return (this.head as BufferListItem).data;
  }

  *[Symbol.iterator]() {
    for (let p = this.head; p; p = p.next) {
      yield p.data;
    }
  }

  // Consumes a specified amount of characters from the buffered data.
  _getString(n: number) {
    let ret = "";
    let p: BufferListItem | null = (this.head as BufferListItem);
    let c = 0;
    p = p.next as BufferListItem;
    do {
      const str = p.data;
      if (n > str.length) {
        ret += str;
        n -= str.length;
      } else {
        if (n === str.length) {
          ret += str;
          ++c;
          if (p.next) {
            this.head = p.next;
          } else {
            this.head = this.tail = null;
          }
        } else {
          ret += str.slice(0, n);
          this.head = p;
          p.data = str.slice(n);
        }
        break;
      }
      ++c;
      p = p.next;
    } while (p);
    this.length -= c;
    return ret;
  }

  // Consumes a specified amount of bytes from the buffered data.
  _getBuffer(n: number) {
    const ret = Buffer.allocUnsafe(n);
    const retLen = n;
    let p: BufferListItem | null = (this.head as BufferListItem);
    let c = 0;
    p = p.next as BufferListItem;
    do {
      const buf = p.data as Buffer;
      if (n > buf.length) {
        ret.set(buf, retLen - n);
        n -= buf.length;
      } else {
        if (n === buf.length) {
          ret.set(buf, retLen - n);
          ++c;
          if (p.next) {
            this.head = p.next;
          } else {
            this.head = this.tail = null;
          }
        } else {
          ret.set(new Uint8Array(buf.buffer, buf.byteOffset, n), retLen - n);
          this.head = p;
          p.data = buf.slice(n);
        }
        break;
      }
      ++c;
      p = p.next;
    } while (p);
    this.length -= c;
    return ret;
  }
}
