// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/textproto/writer_test.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
import { TextProtoWriter } from "./writer.ts";
import { BufWriter } from "../io/bufio.ts";
import { decode, encode } from "../encoding/utf8.ts";
import { assertStrictEquals } from "../testing/asserts.ts";

Deno.test("[textproto] Writer#printLine", async () => {
  const buf = new Deno.Buffer();
  const w = new TextProtoWriter(BufWriter.create(buf));
  await w.printLine("foo 123");
  const written = await Deno.readAll(buf);
  assertStrictEquals(decode(written), "foo 123\r\n");
});

Deno.test("[textproto] DotWriter", async () => {
  const buf = new Deno.Buffer();
  const w = new TextProtoWriter(BufWriter.create(buf));
  const d = w.dotWriter();
  const n = await d.write(encode("abc\n.def\n..ghi\n.jkl\n."));
  assertStrictEquals(n, 21);
  await d.close();
  const written = await Deno.readAll(buf);
  assertStrictEquals(
    decode(written),
    "abc\r\n..def\r\n...ghi\r\n..jkl\r\n..\r\n.\r\n",
  );
});

Deno.test("[textproto] DotWriterCloseEmptyWrite", async () => {
  const buf = new Deno.Buffer();
  const w = new TextProtoWriter(BufWriter.create(buf));
  const d = w.dotWriter();
  const n = await d.write(encode(""));
  assertStrictEquals(n, 0);
  await d.close();
  const written = await Deno.readAll(buf);
  assertStrictEquals(decode(written), "\r\n.\r\n");
});

Deno.test("[textproto] DotWriterCloseNoWrite", async () => {
  const buf = new Deno.Buffer();
  const w = new TextProtoWriter(BufWriter.create(buf));
  const d = w.dotWriter();
  await d.close();
  const written = await Deno.readAll(buf);
  assertStrictEquals(decode(written), "\r\n.\r\n");
});
