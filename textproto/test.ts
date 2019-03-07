// Based on https://github.com/golang/go/blob/891682/src/net/textproto/
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { BufReader } from "../io/bufio.ts";
import { TextProtoReader, append } from "./mod.ts";
import { stringsReader } from "../io/util.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";

function reader(s: string): TextProtoReader {
  return new TextProtoReader(new BufReader(stringsReader(s)));
}

test(async function textprotoReader() {
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

/*
test(async function textprotoReadMIMEHeader() {
	let r = reader("my-key: Value 1  \r\nLong-key: Even \n Longer Value\r\nmy-Key: Value 2\r\n\n");
	let [m, err] = await r.readMIMEHeader();

  console.log("Got headers", m.toString());
	want := MIMEHeader{
		"My-Key":   {"Value 1", "Value 2"},
		"Long-Key": {"Even Longer Value"},
	}
	if !reflect.DeepEqual(m, want) || err != nil {
		t.Fatalf("ReadMIMEHeader: %v, %v; want %v", m, err, want)
	}
});
*/

test(async function textprotoReadMIMEHeaderSingle() {
  let r = reader("Foo: bar\n\n");
  let [m, err] = await r.readMIMEHeader();
  assertEquals(m.get("Foo"), "bar");
  assert(!err);
});

// Test that we read slightly-bogus MIME headers seen in the wild,
// with spaces before colons, and spaces in keys.
test(async function textprotoReadMIMEHeaderNonCompliant() {
  // Invalid HTTP response header as sent by an Axis security
  // camera: (this is handled by IE, Firefox, Chrome, curl, etc.)
  let r = reader(
    "Foo: bar\r\n" +
      "Content-Language: en\r\n" +
      "SID : 0\r\n" +
      // TODO Re-enable Currently fails with:
      // "TypeError: audio mode is not a legal HTTP header name"
      // "Audio Mode : None\r\n" +
      "Privilege : 127\r\n\r\n"
  );
  let [m, err] = await r.readMIMEHeader();
  console.log(m.toString());
  assert(!err);
  /*
	let want = MIMEHeader{
		"Foo":              {"bar"},
		"Content-Language": {"en"},
		"Sid":              {"0"},
		"Audio Mode":       {"None"},
		"Privilege":        {"127"},
	}
	if !reflect.DeepEqual(m, want) || err != nil {
		t.Fatalf("ReadMIMEHeader =\n%v, %v; want:\n%v", m, err, want)
	}
  */
});

test(async function textprotoAppend() {
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const u1 = enc.encode("Hello ");
  const u2 = enc.encode("World");
  const joined = append(u1, u2);
  assertEquals(dec.decode(joined), "Hello World");
});

test(async function textprotoReadEmpty() {
  let r = reader("");
  let [, err] = await r.readMIMEHeader();
  // Should not crash!
  assertEquals(err, "EOF");
});
