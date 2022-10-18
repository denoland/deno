// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

/** **Deprecated**. Use `TextLineStream` from `std/steams` for line-by-line text reading instead.
 *
 * A reader for dealing with low level text based protocols.
 *
 * Based on
 * [net/textproto](https://github.com/golang/go/tree/master/src/net/textproto).
 *
 * @deprecated (will be removed after 0.159.0) Use `TextLineStream` from `std/steams` for line-by-line text reading instead.
 * @module
 */

import type {
  BufReader,
  ReadLineResult,
} from "../../../../test_util/std/io/buffer.ts";
import { concat } from "../../../../test_util/std/bytes/mod.ts";

// Constants created for DRY
const CHAR_SPACE: number = " ".charCodeAt(0);
const CHAR_TAB: number = "\t".charCodeAt(0);
const CHAR_COLON: number = ":".charCodeAt(0);

const WHITESPACES: Array<number> = [CHAR_SPACE, CHAR_TAB];

const decoder = new TextDecoder();

// FROM https://github.com/denoland/deno/blob/b34628a26ab0187a827aa4ebe256e23178e25d39/cli/js/web/headers.ts#L9
const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/g;

function str(buf: Uint8Array | null | undefined): string {
  return !buf ? "" : decoder.decode(buf);
}

/**
 * @deprecated (will be removed after 0.159.0) Use `TextLineStream` from `std/steams` for line-by-line text reading instead.
 */
export class TextProtoReader {
  constructor(readonly r: BufReader) {}

  /** readLine() reads a single line from the TextProtoReader,
   * eliding the final \n or \r\n from the returned string.
   */
  async readLine(): Promise<string | null> {
    const s = await this.readLineSlice();
    return s === null ? null : str(s);
  }

  /** ReadMimeHeader reads a MIME-style header from r.
   * The header is a sequence of possibly continued Key: Value lines
   * ending in a blank line.
   * The returned map m maps CanonicalMIMEHeaderKey(key) to a
   * sequence of values in the same order encountered in the input.
   *
   * For example, consider this input:
   *
   * 	My-Key: Value 1
   * 	Long-Key: Even
   * 	       Longer Value
   * 	My-Key: Value 2
   *
   * Given that input, ReadMIMEHeader returns the map:
   *
   * 	map[string][]string{
   * 		"My-Key": {"Value 1", "Value 2"},
   * 		"Long-Key": {"Even Longer Value"},
   * 	}
   */
  async readMimeHeader(): Promise<Headers | null> {
    const m = new Headers();
    let line: Uint8Array | undefined;

    // The first line cannot start with a leading space.
    let buf = await this.r.peek(1);
    if (buf === null) {
      return null;
    } else if (WHITESPACES.includes(buf[0])) {
      line = (await this.readLineSlice()) as Uint8Array;
    }

    buf = await this.r.peek(1);
    if (buf === null) {
      throw new Deno.errors.UnexpectedEof();
    } else if (WHITESPACES.includes(buf[0])) {
      throw new Deno.errors.InvalidData(
        `malformed MIME header initial line: ${str(line)}`,
      );
    }

    while (true) {
      const kv = await this.readLineSlice(); // readContinuedLineSlice
      if (kv === null) throw new Deno.errors.UnexpectedEof();
      if (kv.byteLength === 0) return m;

      // Key ends at first colon
      let i = kv.indexOf(CHAR_COLON);
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
        (WHITESPACES.includes(kv[i]))
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
    let line = new Uint8Array(0);
    let r: ReadLineResult | null = null;

    do {
      r = await this.r.readLine();
      // TODO(ry):
      // This skipSpace() is definitely misplaced, but I don't know where it
      // comes from nor how to fix it.

      //TODO(SmashingQuasar): Kept skipSpace to preserve behavior but it should be looked into to check if it makes sense when this is used.

      if (r !== null && this.skipSpace(r.line) !== 0) {
        line = concat(line, r.line);
      }
    } while (r !== null && r.more);

    return r === null ? null : line;
  }

  skipSpace(l: Uint8Array): number {
    let n = 0;

    for (const val of l) {
      if (!WHITESPACES.includes(val)) {
        n++;
      }
    }

    return n;
  }
}
