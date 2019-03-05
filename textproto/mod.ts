// Based on https://github.com/golang/go/blob/891682/src/net/textproto/
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { BufReader, BufState } from "../io/bufio.ts";
import { charCode } from "../io/util.ts";

const asciiDecoder = new TextDecoder();
function str(buf: Uint8Array): string {
  if (buf == null) {
    return "";
  } else {
    return asciiDecoder.decode(buf);
  }
}

export class ProtocolError extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "ProtocolError";
  }
}

export function append(a: Uint8Array, b: Uint8Array): Uint8Array {
  if (a == null) {
    return b;
  } else {
    const output = new Uint8Array(a.length + b.length);
    output.set(a, 0);
    output.set(b, a.length);
    return output;
  }
}

export class TextProtoReader {
  constructor(readonly r: BufReader) {}

  /** readLine() reads a single line from the TextProtoReader,
   * eliding the final \n or \r\n from the returned string.
   */
  async readLine(): Promise<[string, BufState]> {
    let [line, err] = await this.readLineSlice();
    return [str(line), err];
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
  async readMIMEHeader(): Promise<[Headers, BufState]> {
    let m = new Headers();
    let line: Uint8Array;

    // The first line cannot start with a leading space.
    let [buf, err] = await this.r.peek(1);
    if (buf[0] == charCode(" ") || buf[0] == charCode("\t")) {
      [line, err] = await this.readLineSlice();
    }

    [buf, err] = await this.r.peek(1);
    if (err == null && (buf[0] == charCode(" ") || buf[0] == charCode("\t"))) {
      throw new ProtocolError(
        `malformed MIME header initial line: ${str(line)}`
      );
    }

    while (true) {
      let [kv, err] = await this.readLineSlice(); // readContinuedLineSlice
      if (kv.byteLength == 0) {
        return [m, err];
      }

      // Key ends at first colon; should not have trailing spaces
      // but they appear in the wild, violating specs, so we remove
      // them if present.
      let i = kv.indexOf(charCode(":"));
      if (i < 0) {
        throw new ProtocolError(`malformed MIME header line: ${str(kv)}`);
      }
      let endKey = i;
      while (endKey > 0 && kv[endKey - 1] == charCode(" ")) {
        endKey--;
      }

      //let key = canonicalMIMEHeaderKey(kv.subarray(0, endKey));
      let key = str(kv.subarray(0, endKey));

      // As per RFC 7230 field-name is a token, tokens consist of one or more chars.
      // We could return a ProtocolError here, but better to be liberal in what we
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
      let value = str(kv.subarray(i));

      m.append(key, value);

      if (err != null) {
        throw err;
      }
    }
  }

  async readLineSlice(): Promise<[Uint8Array, BufState]> {
    // this.closeDot();
    let line: Uint8Array;
    while (true) {
      let [l, more, err] = await this.r.readLine();
      if (err != null) {
        // Go's len(typed nil) works fine, but not in JS
        return [new Uint8Array(0), err];
      }
      // Avoid the copy if the first call produced a full line.
      if (line == null && !more) {
        return [l, null];
      }
      line = append(line, l);
      if (!more) {
        break;
      }
    }
    return [line, null];
  }
}
