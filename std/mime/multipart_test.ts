// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { Buffer, copy, open, remove } = Deno;
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "../testing/asserts.ts";
const { test } = Deno;
import * as path from "../path/mod.ts";
import {
  FormFile,
  MultipartReader,
  MultipartWriter,
  isFormFile,
  matchAfterPrefix,
  scanUntilBoundary,
} from "./multipart.ts";
import { StringWriter } from "../io/writers.ts";

const e = new TextEncoder();
const boundary = "--abcde";
const dashBoundary = e.encode("--" + boundary);
const nlDashBoundary = e.encode("\r\n--" + boundary);

test(function multipartScanUntilBoundary1(): void {
  const data = `--${boundary}`;
  const n = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    true
  );
  assertEquals(n, Deno.EOF);
});

test(function multipartScanUntilBoundary2(): void {
  const data = `foo\r\n--${boundary}`;
  const n = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    true
  );
  assertEquals(n, 3);
});

test(function multipartScanUntilBoundary3(): void {
  const data = `foobar`;
  const n = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    false
  );
  assertEquals(n, data.length);
});

test(function multipartScanUntilBoundary4(): void {
  const data = `foo\r\n--`;
  const n = scanUntilBoundary(
    e.encode(data),
    dashBoundary,
    nlDashBoundary,
    0,
    false
  );
  assertEquals(n, 3);
});

test(function multipartMatchAfterPrefix1(): void {
  const data = `${boundary}\r`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), false);
  assertEquals(v, 1);
});

test(function multipartMatchAfterPrefix2(): void {
  const data = `${boundary}hoge`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), false);
  assertEquals(v, -1);
});

test(function multipartMatchAfterPrefix3(): void {
  const data = `${boundary}`;
  const v = matchAfterPrefix(e.encode(data), e.encode(boundary), false);
  assertEquals(v, 0);
});

test(async function multipartMultipartWriter(): Promise<void> {
  const buf = new Buffer();
  const mw = new MultipartWriter(buf);
  await mw.writeField("foo", "foo");
  await mw.writeField("bar", "bar");
  const f = await open(path.resolve("./mime/testdata/sample.txt"), "r");
  await mw.writeFile("file", "sample.txt", f);
  await mw.close();
  f.close();
});

test(function multipartMultipartWriter2(): void {
  const w = new StringWriter();
  assertThrows(
    (): MultipartWriter => new MultipartWriter(w, ""),
    Error,
    "invalid boundary length"
  );
  assertThrows(
    (): MultipartWriter =>
      new MultipartWriter(
        w,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" +
          "aaaaaaaa"
      ),
    Error,
    "invalid boundary length"
  );
  assertThrows(
    (): MultipartWriter => new MultipartWriter(w, "aaa aaa"),
    Error,
    "invalid boundary character"
  );
  assertThrows(
    (): MultipartWriter => new MultipartWriter(w, "boundary¥¥"),
    Error,
    "invalid boundary character"
  );
});

test(async function multipartMultipartWriter3(): Promise<void> {
  const w = new StringWriter();
  const mw = new MultipartWriter(w);
  await mw.writeField("foo", "foo");
  await mw.close();
  await assertThrowsAsync(
    async (): Promise<void> => {
      await mw.close();
    },
    Error,
    "closed"
  );
  await assertThrowsAsync(
    async (): Promise<void> => {
      // @ts-ignore
      await mw.writeFile("bar", "file", null);
    },
    Error,
    "closed"
  );
  await assertThrowsAsync(
    async (): Promise<void> => {
      await mw.writeField("bar", "bar");
    },
    Error,
    "closed"
  );
  assertThrows(
    (): void => {
      mw.createFormField("bar");
    },
    Error,
    "closed"
  );
  assertThrows(
    (): void => {
      mw.createFormFile("bar", "file");
    },
    Error,
    "closed"
  );
});

test(async function multipartMultipartReader(): Promise<void> {
  // FIXME: path resolution
  const o = await open(path.resolve("./mime/testdata/sample.txt"));
  const mr = new MultipartReader(
    o,
    "--------------------------434049563556637648550474"
  );
  const form = await mr.readForm(10 << 20);
  assertEquals(form["foo"], "foo");
  assertEquals(form["bar"], "bar");
  const file = form["file"] as FormFile;
  assertEquals(isFormFile(file), true);
  assert(file.content !== void 0);
  o.close();
});

test(async function multipartMultipartReader2(): Promise<void> {
  const o = await open(path.resolve("./mime/testdata/sample.txt"));
  const mr = new MultipartReader(
    o,
    "--------------------------434049563556637648550474"
  );
  const form = await mr.readForm(20); //
  try {
    assertEquals(form["foo"], "foo");
    assertEquals(form["bar"], "bar");
    const file = form["file"] as FormFile;
    assertEquals(file.type, "application/octet-stream");
    assert(file.tempfile != null);
    const f = await open(file.tempfile);
    const w = new StringWriter();
    await copy(w, f);
    const json = JSON.parse(w.toString());
    assertEquals(json["compilerOptions"]["target"], "es2018");
    f.close();
  } finally {
    const file = form["file"] as FormFile;
    if (file.tempfile) {
      await remove(file.tempfile);
    }
    o.close();
  }
});
