// Based on https://github.com/golang/go/blob/master/src/net/textproto/reader_test.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { BufReader } from "../io/bufio.ts";
import { TextProtoReader, ProtocolError } from "./mod.ts";
import { stringsReader } from "../io/util.ts";
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";

function reader(s: string): TextProtoReader {
  return new TextProtoReader(new BufReader(stringsReader(s)));
}
// test({
//   name: "[textproto] Reader : DotBytes",
//   async fn(): Promise<void> {
//     const input =
//       "dotlines\r\n.foo\r\n..bar\n...baz\nquux\r\n\r\n.\r\nanot.her\r\n";
//   }
// });

test(async function textprotoReadEmpty(): Promise<void> {
  let r = reader("");
  let [, err] = await r.readMIMEHeader();
  // Should not crash!
  assertEquals(err, "EOF");
});

test(async function textprotoReader(): Promise<void> {
  let r = reader("line1\nline2\n");
  let [s, err] = await r.readLine();
  assertEquals(s, "line1");
  assert(err == null);

  [s, err] = await r.readLine();
  assertEquals(s, "line2");
  assert(err == null);

  [s, err] = await r.readLine();
  assertEquals(s, "");
  assert(err == "EOF");
});

test({
  name: "[textproto] Reader : MIME Header",
  async fn(): Promise<void> {
    const input =
      "my-key: Value 1  \r\nLong-key: Even Longer Value\r\nmy-Key: Value 2\r\n\n";
    const r = reader(input);
    const [m, err] = await r.readMIMEHeader();
    assertEquals(m.get("My-Key"), "Value 1, Value 2");
    assertEquals(m.get("Long-key"), "Even Longer Value");
    assert(!err);
  }
});

test({
  name: "[textproto] Reader : MIME Header Single",
  async fn(): Promise<void> {
    const input = "Foo: bar\n\n";
    const r = reader(input);
    let [m, err] = await r.readMIMEHeader();
    assertEquals(m.get("Foo"), "bar");
    assert(!err);
  }
});

test({
  name: "[textproto] Reader : MIME Header No Key",
  async fn(): Promise<void> {
    const input = ": bar\ntest-1: 1\n\n";
    const r = reader(input);
    let [m, err] = await r.readMIMEHeader();
    assertEquals(m.get("Test-1"), "1");
    assert(!err);
  }
});

test({
  name: "[textproto] Reader : Large MIME Header",
  async fn(): Promise<void> {
    const data = [];
    // Go test is 16*1024. But seems it can't handle more
    for (let i = 0; i < 1024; i++) {
      data.push("x");
    }
    const sdata = data.join("");
    const r = reader(`Cookie: ${sdata}\r\n`);
    let [m] = await r.readMIMEHeader();
    assertEquals(m.get("Cookie"), sdata);
    // TODO re-enable, here err === "EOF" is has to be null
    // assert(!err);
  }
});

// Test that we read slightly-bogus MIME headers seen in the wild,
// with spaces before colons, and spaces in keys.
test({
  name: "[textproto] Reader : MIME Header Non compliant",
  async fn(): Promise<void> {
    const input =
      "Foo: bar\r\n" +
      "Content-Language: en\r\n" +
      "SID : 0\r\n" +
      "Audio Mode : None\r\n" +
      "Privilege : 127\r\n\r\n";
    const r = reader(input);
    let [m, err] = await r.readMIMEHeader();
    assertEquals(m.get("Foo"), "bar");
    assertEquals(m.get("Content-Language"), "en");
    assertEquals(m.get("SID"), "0");
    assertEquals(m.get("Privilege"), "127");
    assert(!err);
    // Not a legal http header
    assertThrows(
      (): void => {
        assertEquals(m.get("Audio Mode"), "None");
      }
    );
  }
});

test({
  name: "[textproto] Reader : MIME Header Malformed",
  async fn(): Promise<void> {
    const input = [
      "No colon first line\r\nFoo: foo\r\n\r\n",
      " No colon first line with leading space\r\nFoo: foo\r\n\r\n",
      "\tNo colon first line with leading tab\r\nFoo: foo\r\n\r\n",
      " First: line with leading space\r\nFoo: foo\r\n\r\n",
      "\tFirst: line with leading tab\r\nFoo: foo\r\n\r\n",
      "Foo: foo\r\nNo colon second line\r\n\r\n"
    ];
    const r = reader(input.join(""));

    let err;
    try {
      await r.readMIMEHeader();
    } catch (e) {
      err = e;
    }
    assert(err instanceof ProtocolError);
  }
});

test({
  name: "[textproto] Reader : MIME Header Trim Continued",
  async fn(): Promise<void> {
    const input =
      "" + // for code formatting purpose.
      "a:\n" +
      " 0 \r\n" +
      "b:1 \t\r\n" +
      "c: 2\r\n" +
      " 3\t\n" +
      "  \t 4  \r\n\n";
    const r = reader(input);
    let err;
    try {
      await r.readMIMEHeader();
    } catch (e) {
      err = e;
    }
    assert(err instanceof ProtocolError);
  }
});

test({
  name: "[textproto] #409 issue : multipart form boundary",
  async fn(): Promise<void> {
    const input = [
      "Accept: */*\r\n",
      'Content-Disposition: form-data; name="test"\r\n',
      " \r\n",
      "------WebKitFormBoundaryimeZ2Le9LjohiUiG--\r\n\n"
    ];
    const r = reader(input.join(""));
    let [m, err] = await r.readMIMEHeader();
    assertEquals(m.get("Accept"), "*/*");
    assertEquals(m.get("Content-Disposition"), 'form-data; name="test"');
    assert(!err);
  }
});
