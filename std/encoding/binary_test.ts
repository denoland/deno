// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
import {
  getNBytes,
  putVarbig,
  putVarnum,
  readVarbig,
  readVarnum,
  sizeof,
  varbig,
  varnum,
  writeVarbig,
  writeVarnum,
  varbigBytes,
  varnumBytes,
} from "./binary.ts";

Deno.test("testGetNBytes", async function (): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await getNBytes(buff, 8);
  assertEquals(rslt, data);
});

Deno.test("testGetNBytesNull", async function (): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await getNBytes(buff, 8);
  assertEquals(rslt, null);
});

Deno.test("testPutVarbig", function (): void {
  const buff = new Uint8Array(8);
  putVarbig({ type: "int64", bytes: buff, value: 0xffeeddccbbaa9988n });
  assertEquals(
    buff,
    new Uint8Array([0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88])
  );
});

Deno.test("testPutVarbigLittleEndian", function (): void {
  const buff = new Uint8Array(8);
  putVarbig({
    type: "int64",
    bytes: buff,
    value: 0x8899aabbccddeeffn,
    endian: "little",
  });
  assertEquals(
    buff,
    new Uint8Array([0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88])
  );
});

Deno.test("testPutVarnum", function (): void {
  const buff = new Uint8Array(4);
  putVarnum({ type: "int32", bytes: buff, value: 0xffeeddcc });
  assertEquals(buff, new Uint8Array([0xff, 0xee, 0xdd, 0xcc]));
});

Deno.test("testPutVarnumLittleEndian", function (): void {
  const buff = new Uint8Array(4);
  putVarnum({
    type: "int32",
    bytes: buff,
    value: 0xccddeeff,
    endian: "little",
  });
  assertEquals(buff, new Uint8Array([0xff, 0xee, 0xdd, 0xcc]));
});

Deno.test("testReadVarbig", async function (): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const reader = new Deno.Buffer(data.buffer);
  const rslt = await readVarbig({ type: "int64", src: reader });
  assertEquals(rslt, 0x0102030405060708n);
});

Deno.test("testReadVarbigLittleEndian", async function (): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const reader = new Deno.Buffer(data.buffer);
  const rslt = await readVarbig({
    type: "int64",
    src: reader,
    endian: "little",
  });
  assertEquals(rslt, 0x0807060504030201n);
});

Deno.test("testReadVarnum", async function (): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4]);
  const reader = new Deno.Buffer(data.buffer);
  const rslt = await readVarnum({ type: "int32", src: reader });
  assertEquals(rslt, 0x01020304);
});

Deno.test("testReadVarnumLittleEndian", async function (): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4]);
  const reader = new Deno.Buffer(data.buffer);
  const rslt = await readVarnum({
    type: "int32",
    src: reader,
    endian: "little",
  });
  assertEquals(rslt, 0x04030201);
});

Deno.test("testSizeof", function (): void {
  assertEquals(1, sizeof("int8"));
  assertEquals(1, sizeof("uint8"));
  assertEquals(2, sizeof("int16"));
  assertEquals(2, sizeof("uint16"));
  assertEquals(4, sizeof("int32"));
  assertEquals(4, sizeof("uint32"));
  assertEquals(8, sizeof("int64"));
  assertEquals(8, sizeof("uint64"));
  assertEquals(4, sizeof("float32"));
  assertEquals(8, sizeof("float64"));
});

Deno.test("testVarbig", function (): void {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const rslt = varbig({ type: "int64", bytes: data });
  assertEquals(rslt, 0x0102030405060708n);
});

Deno.test("testVarbigLittleEndian", function (): void {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const rslt = varbig({ type: "int64", bytes: data, endian: "little" });
  assertEquals(rslt, 0x0807060504030201n);
});

Deno.test("testVarnum", function (): void {
  const data = new Uint8Array([1, 2, 3, 4]);
  const rslt = varnum({ type: "int32", bytes: data });
  assertEquals(rslt, 0x01020304);
});
Deno.test("testVarnumLittleEndian", function (): void {
  const data = new Uint8Array([1, 2, 3, 4]);
  const rslt = varnum({ type: "int32", bytes: data, endian: "little" });
  assertEquals(rslt, 0x04030201);
});

Deno.test("testWriteVarbig", async function (): Promise<void> {
  const data = new Uint8Array(8);
  const writer = new Deno.Buffer();
  await writeVarbig({ type: "int64", value: 0x0102030405060708n, dst: writer });
  await writer.read(data);
  assertEquals(
    data,
    new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
  );
});

Deno.test("testWriteVarbigLittleEndian", async function (): Promise<void> {
  const data = new Uint8Array(8);
  const writer = new Deno.Buffer();
  await writeVarbig({
    type: "int64",
    value: 0x0807060504030201n,
    dst: writer,
    endian: "little",
  });
  await writer.read(data);
  assertEquals(
    data,
    new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
  );
});

Deno.test("testWriteVarnum", async function (): Promise<void> {
  const data = new Uint8Array(4);
  const writer = new Deno.Buffer();
  await writeVarnum({ type: "int32", value: 0x01020304, dst: writer });
  await writer.read(data);
  assertEquals(data, new Uint8Array([0x01, 0x02, 0x03, 0x04]));
});

Deno.test("testWriteVarnumLittleEndian", async function (): Promise<void> {
  const data = new Uint8Array(4);
  const writer = new Deno.Buffer();
  await writeVarnum({
    type: "int32",
    value: 0x04030201,
    dst: writer,
    endian: "little",
  });
  await writer.read(data);
  assertEquals(data, new Uint8Array([0x01, 0x02, 0x03, 0x04]));
});

Deno.test("testVarbigBytes", function (): void {
  const rslt = varbigBytes({ type: "int64", value: 0x0102030405060708n });
  assertEquals(
    rslt,
    new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
  );
});

Deno.test("testVarbigBytesLittleEndian", async function (): Promise<void> {
  const rslt = varbigBytes({
    type: "int64",
    value: 0x0807060504030201n,
    endian: "little",
  });
  assertEquals(
    rslt,
    new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
  );
});

Deno.test("testVarnumBytes", async function (): Promise<void> {
  const rslt = varnumBytes({ type: "int32", value: 0x01020304 });
  assertEquals(rslt, new Uint8Array([0x01, 0x02, 0x03, 0x04]));
});

Deno.test("testVarnumBytesLittleEndian", async function (): Promise<void> {
  const rslt = varnumBytes({
    type: "int32",
    value: 0x04030201,
    endian: "little",
  });
  assertEquals(rslt, new Uint8Array([0x01, 0x02, 0x03, 0x04]));
});
