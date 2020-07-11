//@ts-nocheck
//TODO@Soremwar
//Definetely stilize this
//Add tests
//Add types
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { Buffer } from "./buffer.ts";
import {
  normalizeEncoding as castEncoding,
  notImplemented
} from "./_utils.ts";

enum NotImplemented {
  "ascii",
  "latin1",
  "utf16le",
}

function normalizeEncoding(enc: string): string {
  const encoding = castEncoding(enc);
  if(encoding && encoding in NotImplemented) notImplemented(encoding);
  if (!encoding &&  enc.toLowerCase() !== "raw") throw new Error(`Unknown encoding: ${enc}`);
  return encoding;
}
/*
* Checks the type of a UTF-8 byte, whether it's ASCII, a leading byte, or a
* continuation byte. If an invalid byte is detected, -2 is returned.
* */
function utf8CheckByte(byte: number): number {
  if (byte <= 0x7f) return 0;
  else if (byte >> 5 === 0x06) return 2;
  else if (byte >> 4 === 0x0e) return 3;
  else if (byte >> 3 === 0x1e) return 4;
  return byte >> 6 === 0x02 ? -1 : -2;
}

/*
* Checks at most 3 bytes at the end of a Buffer in order to detect an
* incomplete multi-byte UTF-8 character. The total number of bytes (2, 3, or 4)
* needed to complete the UTF-8 character (if applicable) are returned.
* */
function utf8CheckIncomplete(self: StringDecoder, buf: Buffer, i: number) {
  let j = buf.length - 1;
  if (j < i) return 0;
  let nb = utf8CheckByte(buf[j]);
  if (nb >= 0) {
    if (nb > 0) self.lastNeed = nb - 1;
    return nb;
  }
  if (--j < i || nb === -2) return 0;
  nb = utf8CheckByte(buf[j]);
  if (nb >= 0) {
    if (nb > 0) self.lastNeed = nb - 2;
    return nb;
  }
  if (--j < i || nb === -2) return 0;
  nb = utf8CheckByte(buf[j]);
  if (nb >= 0) {
    if (nb > 0) {
      if (nb === 2) nb = 0;
      else self.lastNeed = nb - 3;
    }
    return nb;
  }
  return 0;
}

/*
* Validates as many continuation bytes for a multi-byte UTF-8 character as
* needed or are available. If we see a non-continuation byte where we expect
* one, we "replace" the validated continuation bytes we've seen so far with
* a single UTF-8 replacement character ('\ufffd'), to match v8's UTF-8 decoding
* behavior. The continuation byte check is included three times in the case
* where all of the continuation bytes for a character exist in the same buffer.
* It is also done this way as a slight performance increase instead of using a
* loop.
* */
function utf8CheckExtraBytes(self: StringDecoder, buf: Buffer) {
  if ((buf[0] & 0xc0) !== 0x80) {
    self.lastNeed = 0;
    return "\ufffd";
  }
  if (self.lastNeed > 1 && buf.length > 1) {
    if ((buf[1] & 0xc0) !== 0x80) {
      self.lastNeed = 1;
      return "\ufffd";
    }
    if (self.lastNeed > 2 && buf.length > 2) {
      if ((buf[2] & 0xc0) !== 0x80) {
        self.lastNeed = 2;
        return "\ufffd";
      }
    }
  }
}

/*
* Attempts to complete a multi-byte UTF-8 character using bytes from a Buffer.
* */
function utf8FillLastComplete(buf: Buffer): string | undefined {
  const p = this.lastTotal - this.lastNeed;
  const r = utf8CheckExtraBytes(this, buf);
  if (r !== undefined) return r;
  if (this.lastNeed <= buf.length) {
    buf.copy(this.lastChar, p, 0, this.lastNeed);
    return this.lastChar.toString(this.encoding, 0, this.lastTotal);
  }
  buf.copy(this.lastChar, p, 0, buf.length);
  this.lastNeed -= buf.length;
}

/*
* Attempts to complete a partial non-UTF-8 character using bytes from a Buffer
* */
function utf8FillLastIncomplete(buf: Buffer): string {
  if (this.lastNeed <= buf.length) {
    buf.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, this.lastNeed);
    return this.lastChar.toString(this.encoding, 0, this.lastTotal);
  }
  buf.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, buf.length);
  this.lastNeed -= buf.length;
};

/*
* Returns all complete UTF-8 characters in a Buffer. If the Buffer ended on a
* partial character, the character's bytes are buffered until the required
* number of bytes are available.
* */
function utf8Text(buf: Buffer, i: number): string {
  const total = utf8CheckIncomplete(this, buf, i);
  if (!this.lastNeed) return buf.toString("utf8", i);
  this.lastTotal = total;
  const end = buf.length - (total - this.lastNeed);
  buf.copy(this.lastChar, 0, end);
  return buf.toString("utf8", i, end);
}

/*
* For UTF-8, a replacement character is added when ending on a partial
* character.
* */
function utf8End(buf: Buffer): string {
  const r = buf && buf.length ? this.write(buf) : "";
  if (this.lastNeed) return r + "\ufffd";
  return r;
}

function utf8Write (buf: Buffer) {
  if (buf.length === 0) return "";
  let r;
  let i;
  if (this.lastNeed) {
    r = this.fillLast(buf);
    if (r === undefined) return "";
    i = this.lastNeed;
    this.lastNeed = 0;
  } else {
    i = 0;
  }
  if (i < buf.length) return r ? r + this.text(buf, i) : this.text(buf, i);
  return r || "";
}

function base64Text(buf: Buffer, i: number): string {
  const n = (buf.length - i) % 3;
  if (n === 0) return buf.toString("base64", i);
  this.lastNeed = 3 - n;
  this.lastTotal = 3;
  if (n === 1) {
    this.lastChar[0] = buf[buf.length - 1];
  } else {
    this.lastChar[0] = buf[buf.length - 2];
    this.lastChar[1] = buf[buf.length - 1];
  }
  return buf.toString("base64", i, buf.length - n);
}

function base64End(buf: Buffer): string {
  const r = buf && buf.length ? this.write(buf) : "";
  if (this.lastNeed)
    return r + this.lastChar.toString("base64", 0, 3 - this.lastNeed);
  return r;
}

function simpleWrite(buf: Buffer): string {
  return buf.toString(this.encoding);
}

function simpleEnd(buf: Buffer): string {
  return buf && buf.length ? this.write(buf) : "";
}

/*
* StringDecoder provides an interface for efficiently splitting a series of
* buffers into a series of JS strings without breaking apart multi-byte
* characters.
* */
export function StringDecoder(encoding) {
  this.encoding = normalizeEncoding(encoding);
  let nb;
  switch (this.encoding) {
    case "utf8":
      this.fillLast = utf8FillLastComplete;
      nb = 4;
      break;
    case "base64":
      this.text = base64Text;
      this.end = base64End;
      nb = 3;
      break;
    default:
      this.write = simpleWrite;
      this.end = simpleEnd;
      return;
  }
  this.lastNeed = 0;
  this.lastTotal = 0;
  this.lastChar = Buffer.allocUnsafe(nb);
}

StringDecoder.prototype.end = utf8End;
StringDecoder.prototype.fillLast = utf8FillLastIncomplete;
StringDecoder.prototype.text = utf8Text;
StringDecoder.prototype.write = utf8Write;
