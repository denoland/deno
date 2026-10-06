// Copyright 2018-2026 the Deno authors. MIT license.

import { closeSync, fstatSync, openSync, readFileSync } from "node:fs";
import { Buffer } from "node:buffer";
import { assertEquals, loadTestLibrary } from "./common.js";

const uv = loadTestLibrary();

Deno.test("napi uv_get_osfhandle returns a borrowed OS handle", () => {
  const path = new URL(import.meta.url);
  const expected = readFileSync(path, "utf8");
  const fd = openSync(path, "r");
  try {
    assertEquals(uv.test_uv_get_osfhandle(fd), expected);
    assertEquals(fstatSync(fd).size, Buffer.byteLength(expected));
  } finally {
    closeSync(fd);
  }
});

Deno.test("napi uv_get_osfhandle handles invalid descriptors", () => {
  assertEquals(uv.test_uv_get_osfhandle(-1), null);
  if (Deno.build.os === "windows") {
    assertEquals(uv.test_uv_get_osfhandle(0x3fffffff), null);
    const fd = openSync(new URL(import.meta.url), "r");
    closeSync(fd);
    assertEquals(uv.test_uv_get_osfhandle(fd), null);
  }
});
