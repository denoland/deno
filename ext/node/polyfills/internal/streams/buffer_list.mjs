// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file

import { Buffer } from "node:buffer";
import { inspect } from "ext:deno_node/internal/util/inspect.mjs";

class BufferList {
  constructor() {
    this.head = null;
    this.tail = null;
    this.length = 0;
  }

  push(v) {
    const entry = { data: v, next: null };
    if (this.length > 0) {
      this.tail.next = entry;
    } else {
      this.head = entry;
    }
    this.tail = entry;
    ++this.length;
  }

  unshift(v) {
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
    const ret = this.head.data;
    if (this.length === 1) {
      this.head = this.tail = null;
    } else {
      this.head = this.head.next;
    }
    --this.length;
    return ret;
  }

  clear() {
    this.head = this.tail = null;
    this.length = 0;
  }

  join(s) {
    if (this.length === 0) {
      return "";
    }
    let p = this.head;
    let ret = "" + p.data;
    while (p = p.next) {
      ret += s + p.data;
    }
    return ret;
  }

  concat(n) {
    if (this.length === 0) {
      return Buffer.alloc(0);
    }
    const ret = Buffer.allocUnsafe(n >>> 0);
    let p = this.head;
    let i = 0;
    while (p) {
      ret.set(p.data, i);
      i += p.data.length;
      p = p.next;
    }
    return ret;
  }

  // Consumes a specified amount of bytes or characters from the buffered data.
  consume(n, hasStrings) {
    const data = this.head.data;
    if (n < data.length) {
      // `slice` is the same for buffers and strings.
      const slice = data.slice(0, n);
      this.head.data = data.slice(n);
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
    return this.head.data;
  }

  *[Symbol.iterator]() {
    for (let p = this.head; p; p = p.next) {
      yield p.data;
    }
  }

  // Consumes a specified amount of characters from the buffered data.
  _getString(n) {
    let ret = "";
    let p = this.head;
    let c = 0;
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
    } while (p = p.next);
    this.length -= c;
    return ret;
  }

  // Consumes a specified amount of bytes from the buffered data.
  _getBuffer(n) {
    const ret = Buffer.allocUnsafe(n);
    const retLen = n;
    let p = this.head;
    let c = 0;
    do {
      const buf = p.data;
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
          ret.set(
            new Uint8Array(buf.buffer, buf.byteOffset, n),
            retLen - n,
          );
          this.head = p;
          p.data = buf.slice(n);
        }
        break;
      }
      ++c;
    } while (p = p.next);
    this.length -= c;
    return ret;
  }

  // Make sure the linked list only shows the minimal necessary information.
  [inspect.custom](_, options) {
    return inspect(this, {
      ...options,
      // Only inspect one level.
      depth: 0,
      // It should not recurse.
      customInspect: false,
    });
  }
}

export default BufferList;
