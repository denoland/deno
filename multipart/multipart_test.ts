// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

const { Buffer, copy, open, remove } = Deno;
import {
  assert,
  assertEq,
  assertThrows,
  assertThrowsAsync
} from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import {
  matchAfterPrefix,
  MultipartReader,
  MultipartWriter,
  scanUntilBoundary
} from "./multipart.ts";
import * as path from "../fs/path.ts";
import { FormFile, isFormFile } from "./formfile.ts";
import { StringWriter } from "../io/writers.ts";

const e = new TextEncoder();
const boundary = "--abcde";
const dashBoundary = e.encode("--" + boundary);
const nlDashBoundary = e.encode("\r\n--" + boundary);

test(function multipartScanUntilBoundary1() {
  const data = `--${boundary}`;
  const [n, err] = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    "EOF"
  );
  assertEq(n, 0);
  assertEq(err, "EOF");
});

test(function multipartScanUntilBoundary2() {
  const data = `foo\r\n--${boundary}`;
  const [n, err] = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    "EOF"
  );
  assertEq(n, 3);
  assertEq(err, "EOF");
});

test(function multipartScanUntilBoundary4() {
  const data = `foo\r\n--`;
  const [n, err] = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    null
  );
  assertEq(n, 3);
  assertEq(err, null);
});

test(function multipartScanUntilBoundary3() {
  const data = `foobar`;
  const [n, err] = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    null
  );
  assertEq(n, data.length);
  assertEq(err, null);
});

test(function multipartMatchAfterPrefix1() {
  const data = `${boundary}\r`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), null);
  assertEq(v, 1);
});

test(function multipartMatchAfterPrefix2() {
  const data = `${boundary}hoge`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), null);
  assertEq(v, -1);
});

test(function multipartMatchAfterPrefix3() {
  const data = `${boundary}`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), null);
  assertEq(v, 0);
});

test(async function multipartMultipartWriter() {
  const buf = new Buffer();
  const mw = new MultipartWriter(buf);
  await mw.writeField("foo", "foo");
  await mw.writeField("bar", "bar");
  const f = await open(path.resolve("./multipart/fixtures/sample.txt"), "r");
  await mw.writeFile("file", "sample.txt", f);
  await mw.close();
});

test(function multipartMultipartWriter2() {
  const w = new StringWriter();
  assertThrows(
    () => new MultipartWriter(w, ""),
    Error,
    "invalid boundary length"
  );
  assertThrows(
    () =>
      new MultipartWriter(
        w,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
      ),
    Error,
    "invalid boundary length"
  );
  assertThrows(
    () => new MultipartWriter(w, "aaa aaa"),
    Error,
    "invalid boundary character"
  );
  assertThrows(
    () => new MultipartWriter(w, "boundary¥¥"),
    Error,
    "invalid boundary character"
  );
});

test(async function multipartMultipartWriter3() {
  const w = new StringWriter();
  const mw = new MultipartWriter(w);
  await mw.writeField("foo", "foo");
  await mw.close();
  await assertThrowsAsync(
    async () => {
      await mw.close();
    },
    Error,
    "closed"
  );
  await assertThrowsAsync(
    async () => {
      await mw.writeFile("bar", "file", null);
    },
    Error,
    "closed"
  );
  await assertThrowsAsync(
    async () => {
      await mw.writeField("bar", "bar");
    },
    Error,
    "closed"
  );
  assertThrows(
    () => {
      mw.createFormField("bar");
    },
    Error,
    "closed"
  );
  assertThrows(
    () => {
      mw.createFormFile("bar", "file");
    },
    Error,
    "closed"
  );
});

test(async function multipartMultipartReader() {
  // FIXME: path resolution
  const o = await open(path.resolve("./multipart/fixtures/sample.txt"));
  const mr = new MultipartReader(
    o,
    "--------------------------434049563556637648550474"
  );
  const form = await mr.readForm(10 << 20);
  assertEq(form["foo"], "foo");
  assertEq(form["bar"], "bar");
  const file = form["file"] as FormFile;
  assertEq(isFormFile(file), true);
  assert(file.content !== void 0);
});

test(async function multipartMultipartReader2() {
  const o = await open(path.resolve("./multipart/fixtures/sample.txt"));
  const mr = new MultipartReader(
    o,
    "--------------------------434049563556637648550474"
  );
  const form = await mr.readForm(20); //
  try {
    assertEq(form["foo"], "foo");
    assertEq(form["bar"], "bar");
    const file = form["file"] as FormFile;
    assertEq(file.type, "application/octet-stream");
    const f = await open(file.tempfile);
    const w = new StringWriter();
    await copy(w, f);
    const json = JSON.parse(w.toString());
    assertEq(json["compilerOptions"]["target"], "es2018");
    f.close();
  } finally {
    const file = form["file"] as FormFile;
    await remove(file.tempfile);
  }
});
