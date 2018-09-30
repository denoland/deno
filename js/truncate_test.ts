// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEqual } from "./test_util.ts";
import * as deno from "deno";

function readDataSync(name: string): string {
  const data = deno.readFileSync(name);
  const decoder = new TextDecoder("utf-8");
  const text = decoder.decode(data);
  return text;
}

async function readData(name: string): Promise<string> {
  const data = await deno.readFile(name);
  const decoder = new TextDecoder("utf-8");
  const text = decoder.decode(data);
  return text;
}

testPerm({ write: true }, function truncateSyncSuccess() {
  const enc = new TextEncoder();
  const d = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test_truncateSync.txt";
  deno.writeFileSync(filename, d);
  deno.truncateSync(filename, 20);
  let data = readDataSync(filename);
  assertEqual(data.length, 20);
  deno.truncateSync(filename, 5);
  data = readDataSync(filename);
  assertEqual(data.length, 5);
  deno.truncateSync(filename, -5);
  data = readDataSync(filename);
  assertEqual(data.length, 0);
  deno.removeSync(filename);
});

testPerm({ write: true }, async function truncateSuccess() {
  const enc = new TextEncoder();
  const d = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test_truncate.txt";
  await deno.writeFile(filename, d);
  await deno.truncate(filename, 20);
  let data = await readData(filename);
  assertEqual(data.length, 20);
  await deno.truncate(filename, 5);
  data = await readData(filename);
  assertEqual(data.length, 5);
  await deno.truncate(filename, -5);
  data = await readData(filename);
  assertEqual(data.length, 0);
  await deno.remove(filename);
});

testPerm({ write: false }, function truncateSyncPerm() {
  let err;
  try {
    deno.mkdirSync("/test_truncateSyncPermission.txt");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: false }, async function truncatePerm() {
  let err;
  try {
    await deno.mkdir("/test_truncatePermission.txt");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});
