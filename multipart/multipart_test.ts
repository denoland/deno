// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

const { Buffer, copy, open, remove } = Deno;
import { assert, test } from "../testing/mod.ts";
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
const d = new TextDecoder();
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
  assert.equal(n, 0);
  assert.equal(err, "EOF");
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
  assert.equal(n, 3);
  assert.equal(err, "EOF");
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
  assert.equal(n, 3);
  assert.equal(err, null);
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
  assert.equal(n, data.length);
  assert.equal(err, null);
});

test(function multipartMatchAfterPrefix1() {
  const data = `${boundary}\r`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), null);
  assert.equal(v, 1);
});

test(function multipartMatchAfterPrefix2() {
  const data = `${boundary}hoge`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), null);
  assert.equal(v, -1);
});

test(function multipartMatchAfterPrefix3() {
  const data = `${boundary}`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), null);
  assert.equal(v, 0);
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
  assert.throws(
    () => new MultipartWriter(w, ""),
    Error,
    "invalid boundary length"
  );
  assert.throws(
    () =>
      new MultipartWriter(
        w,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
      ),
    Error,
    "invalid boundary length"
  );
  assert.throws(
    () => new MultipartWriter(w, "aaa aaa"),
    Error,
    "invalid boundary character"
  );
  assert.throws(
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
  await assert.throwsAsync(
    async () => {
      await mw.close();
    },
    Error,
    "closed"
  );
  await assert.throwsAsync(
    async () => {
      await mw.writeFile("bar", "file", null);
    },
    Error,
    "closed"
  );
  await assert.throwsAsync(
    async () => {
      await mw.writeField("bar", "bar");
    },
    Error,
    "closed"
  );
  assert.throws(
    () => {
      mw.createFormField("bar");
    },
    Error,
    "closed"
  );
  assert.throws(
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
  assert.equal(form["foo"], "foo");
  assert.equal(form["bar"], "bar");
  const file = form["file"] as FormFile;
  assert.equal(isFormFile(file), true);
  assert.assert(file.content !== void 0);
});

test(async function multipartMultipartReader2() {
  const o = await open(path.resolve("./multipart/fixtures/sample.txt"));
  const mr = new MultipartReader(
    o,
    "--------------------------434049563556637648550474"
  );
  const form = await mr.readForm(20); //
  try {
    assert.equal(form["foo"], "foo");
    assert.equal(form["bar"], "bar");
    const file = form["file"] as FormFile;
    assert.equal(file.type, "application/octet-stream");
    const f = await open(file.tempfile);
    const w = new StringWriter();
    await copy(w, f);
    const json = JSON.parse(w.toString());
    assert.equal(json["compilerOptions"]["target"], "es2018");
    f.close();
  } finally {
    const file = form["file"] as FormFile;
    await remove(file.tempfile);
  }
});
