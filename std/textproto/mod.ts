// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/tree/master/src/net/textproto
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import type { BufReader } from "../io/bufio.ts";
import { concat } from "../bytes/mod.ts";
import { decode } from "../encoding/utf8.ts";

// FROM https://github.com/denoland/deno/blob/b34628a26ab0187a827aa4ebe256e23178e25d39/cli/js/web/headers.ts#L9
const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/g;

function str(buf: Uint8Array | null | undefined): string {
  if (buf == null) {
    return "";
  } else {
    return decode(buf);
  }
}

function charCode(s: string): number {
  return s.charCodeAt(0);
}

export class TextProtoReader {
  constructor(readonly r: BufReader) {}

  /** readLine() reads a single line from the TextProtoReader,
   * eliding the final \n or \r\n from the returned string.
   */
  async readLine(): Promise<string | null> {
    const s = await this.readLineSlice();
    if (s === null) return null;
    return str(s);
  }

  /** ReadMIMEHeader reads a MIME-style header from r.
   * The header is a sequence of possibly continued Key: Value lines
   * ending in a blank line.
   * The returned map m maps CanonicalMIMEHeaderKey(key) to a
   * sequence of values in the same order encountered in the input.
   *
   * For example, consider this input:
   *
   *	My-Key: Value 1
   *	Long-Key: Even
   *	       Longer Value
   *	My-Key: Value 2
   *
   * Given that input, ReadMIMEHeader returns the map:
   *
   *	map[string][]string{
   *		"My-Key": {"Value 1", "Value 2"},
   *		"Long-Key": {"Even Longer Value"},
   *	}
   */
  async readMIMEHeader(): Promise<Headers | null> {
    const m = new Headers();
    let line: Uint8Array | undefined;

    // The first line cannot start with a leading space.
    let buf = await this.r.peek(1);
    if (buf === null) {
      return null;
    } else if (buf[0] == charCode(" ") || buf[0] == charCode("\t")) {
      line = (await this.readLineSlice()) as Uint8Array;
    }

    buf = await this.r.peek(1);
    if (buf === null) {
      throw new Deno.errors.UnexpectedEof();
    } else if (buf[0] == charCode(" ") || buf[0] == charCode("\t")) {
      throw new Deno.errors.InvalidData(
        `malformed MIME header initial line: ${str(line)}`,
      );
    }

    while (true) {
      const kv = await this.readLineSlice(); // readContinuedLineSlice
      if (kv === null) throw new Deno.errors.UnexpectedEof();
      if (kv.byteLength === 0) return m;

      // Key ends at first colon
      let i = kv.indexOf(charCode(":"));
      if (i < 0) {
        throw new Deno.errors.InvalidData(
          `malformed MIME header line: ${str(kv)}`,
        );
      }

      //let key = canonicalMIMEHeaderKey(kv.subarray(0, endKey));
      const key = str(kv.subarray(0, i));

      // As per RFC 7230 field-name is a token,
      // tokens consist of one or more chars.
      // We could throw `Deno.errors.InvalidData` here,
      // but better to be liberal in what we
      // accept, so if we get an empty key, skip it.
      if (key == "") {
        continue;
      }

      // Skip initial spaces in value.
      i++; // skip colon
      while (
        i < kv.byteLength &&
        (kv[i] == charCode(" ") || kv[i] == charCode("\t"))
      ) {
        i++;
      }
      const value = str(kv.subarray(i)).replace(
        invalidHeaderCharRegex,
        encodeURI,
      );

      // In case of invalid header we swallow the error
      // example: "Audio Mode" => invalid due to space in the key
      try {
        m.append(key, value);
      } catch {
        // Pass
      }
    }
  }

  async readLineSlice(): Promise<Uint8Array | null> {
    // this.closeDot();
    let line: Uint8Array | undefined;
    while (true) {
      const r = await this.r.readLine();
      if (r === null) return null;
      const { line: l, more } = r;

      // Avoid the copy if the first call produced a full line.
      if (!line && !more) {
        // TODO(ry):
        // This skipSpace() is definitely misplaced, but I don't know where it
        // comes from nor how to fix it.
        if (this.skipSpace(l) === 0) {
          return new Uint8Array(0);
        }
        return l;
      }
      line = line ? concat(line, l) : l;
      if (!more) {
        break;
      }
    }
    return line;
  }

  skipSpace(l: Uint8Array): number {
    let n = 0;
    for (let i = 0; i < l.length; i++) {
      if (l[i] === charCode(" ") || l[i] === charCode("\t")) {
        continue;
      }
      n++;
    }
    return n;
  }
}
