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
} from "./binary.ts";

Deno.test(async function testGetNBytes(): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await getNBytes(buff, 8);
  assertEquals(rslt, data);
});

Deno.test(async function testGetNBytesThrows(): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4]);
  const buff = new Deno.Buffer(data.buffer);
  await assertThrowsAsync(async () => {
    await getNBytes(buff, 8);
  }, Deno.errors.UnexpectedEof);
});

Deno.test(function testPutVarbig(): void {
  const buff = new Uint8Array(8);
  putVarbig(buff, 0xffeeddccbbaa9988n);
  assertEquals(
    buff,
    new Uint8Array([0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88])
  );
});

Deno.test(function testPutVarbigLittleEndian(): void {
  const buff = new Uint8Array(8);
  putVarbig(buff, 0x8899aabbccddeeffn, { endian: "little" });
  assertEquals(
    buff,
    new Uint8Array([0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88])
  );
});

Deno.test(function testPutVarnum(): void {
  const buff = new Uint8Array(4);
  putVarnum(buff, 0xffeeddcc);
  assertEquals(buff, new Uint8Array([0xff, 0xee, 0xdd, 0xcc]));
});

Deno.test(function testPutVarnumLittleEndian(): void {
  const buff = new Uint8Array(4);
  putVarnum(buff, 0xccddeeff, { endian: "little" });
  assertEquals(buff, new Uint8Array([0xff, 0xee, 0xdd, 0xcc]));
});

Deno.test(async function testReadVarbig(): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await readVarbig(buff);
  assertEquals(rslt, 0x0102030405060708n);
});

Deno.test(async function testReadVarbigLittleEndian(): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await readVarbig(buff, { endian: "little" });
  assertEquals(rslt, 0x0807060504030201n);
});

Deno.test(async function testReadVarnum(): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await readVarnum(buff);
  assertEquals(rslt, 0x01020304);
});

Deno.test(async function testReadVarnumLittleEndian(): Promise<void> {
  const data = new Uint8Array([1, 2, 3, 4]);
  const buff = new Deno.Buffer(data.buffer);
  const rslt = await readVarnum(buff, { endian: "little" });
  assertEquals(rslt, 0x04030201);
});

Deno.test(function testSizeof(): void {
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

Deno.test(function testVarbig(): void {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const rslt = varbig(data);
  assertEquals(rslt, 0x0102030405060708n);
});

Deno.test(function testVarbigLittleEndian(): void {
  const data = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
  const rslt = varbig(data, { endian: "little" });
  assertEquals(rslt, 0x0807060504030201n);
});

Deno.test(function testVarnum(): void {
  const data = new Uint8Array([1, 2, 3, 4]);
  const rslt = varnum(data);
  assertEquals(rslt, 0x01020304);
});
Deno.test(function testVarnumLittleEndian(): void {
  const data = new Uint8Array([1, 2, 3, 4]);
  const rslt = varnum(data, { endian: "little" });
  assertEquals(rslt, 0x04030201);
});

Deno.test(async function testWriteVarbig(): Promise<void> {
  const data = new Uint8Array(8);
  const buff = new Deno.Buffer();
  await writeVarbig(buff, 0x0102030405060708n);
  await buff.read(data);
  assertEquals(
    data,
    new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
  );
});

Deno.test(async function testWriteVarbigLittleEndian(): Promise<void> {
  const data = new Uint8Array(8);
  const buff = new Deno.Buffer();
  await writeVarbig(buff, 0x0807060504030201n, { endian: "little" });
  await buff.read(data);
  assertEquals(
    data,
    new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
  );
});

Deno.test(async function testWriteVarnum(): Promise<void> {
  const data = new Uint8Array(4);
  const buff = new Deno.Buffer();
  await writeVarnum(buff, 0x01020304);
  await buff.read(data);
  assertEquals(data, new Uint8Array([0x01, 0x02, 0x03, 0x04]));
});

Deno.test(async function testWriteVarnumLittleEndian(): Promise<void> {
  const data = new Uint8Array(4);
  const buff = new Deno.Buffer();
  await writeVarnum(buff, 0x04030201, { endian: "little" });
  await buff.read(data);
  assertEquals(data, new Uint8Array([0x01, 0x02, 0x03, 0x04]));
});
