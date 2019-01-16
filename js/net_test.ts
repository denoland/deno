// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as deno from "deno";
import { testPerm, assert, assertEqual } from "./test_util.ts";
import { deferred } from "deno";

testPerm({ net: true }, function netListenClose() {
  const listener = deno.listen("tcp", "127.0.0.1:4500");
  listener.close();
});

testPerm({ net: true }, async function netCloseWhileAccept() {
  const listener = deno.listen("tcp", ":4501");
  const p = listener.accept();
  listener.close();
  let err;
  try {
    await p;
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEqual(err.kind, deno.ErrorKind.Other);
  assertEqual(err.message, "Listener has been closed");
});

testPerm({ net: true }, async function netConcurrentAccept() {
  const listener = deno.listen("tcp", ":4502");
  let err;
  // Consume this accept error
  // (since it would still be waiting when listener.close is called)
  listener.accept().catch(e => {
    assertEqual(e.kind, deno.ErrorKind.Other);
    assertEqual(e.message, "Listener has been closed");
  });
  const p1 = listener.accept();
  try {
    await p1;
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEqual(err.kind, deno.ErrorKind.Other);
  assertEqual(err.message, "Another accept task is ongoing");
  listener.close();
});

testPerm({ net: true }, async function netDialListen() {
  const listener = deno.listen("tcp", ":4500");
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    conn.close();
  });
  const conn = await deno.dial("tcp", "127.0.0.1:4500");
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEqual(3, readResult.nread);
  assertEqual(1, buf[0]);
  assertEqual(2, buf[1]);
  assertEqual(3, buf[2]);
  assert(conn.rid > 0);

  // TODO Currently ReadResult does not properly transmit EOF in the same call.
  // it requires a second call to get the EOF. Either ReadResult to be an
  // integer in which 0 signifies EOF or the handler should be modified so that
  // EOF is properly transmitted.
  assertEqual(false, readResult.eof);

  const readResult2 = await conn.read(buf);
  assertEqual(true, readResult2.eof);

  listener.close();
  conn.close();
});

/* TODO Fix broken test.
testPerm({ net: true }, async function netCloseReadSuccess() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);
  const closeDeferred = deferred();
  const closeReadDeferred = deferred();
  listener.accept().then(async conn => {
    await closeReadDeferred.promise;
    await conn.write(new Uint8Array([1, 2, 3]));
    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assertEqual(3, readResult.nread);
    assertEqual(4, buf[0]);
    assertEqual(5, buf[1]);
    assertEqual(6, buf[2]);
    conn.close();
    closeDeferred.resolve();
  });
  const conn = await deno.dial("tcp", addr);
  conn.closeRead(); // closing read
  closeReadDeferred.resolve();
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEqual(0, readResult.nread); // No error, read nothing
  assertEqual(true, readResult.eof); // with immediate EOF
  // Ensure closeRead does not impact write
  await conn.write(new Uint8Array([4, 5, 6]));
  await closeDeferred.promise;
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netDoubleCloseRead() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await deno.dial("tcp", addr);
  conn.closeRead(); // closing read
  let err;
  try {
    // Duplicated close should throw error
    conn.closeRead();
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEqual(err.kind, deno.ErrorKind.NotConnected);
  assertEqual(err.name, "NotConnected");
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netCloseWriteSuccess() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await deno.dial("tcp", addr);
  conn.closeWrite(); // closing write
  const buf = new Uint8Array(1024);
  // Check read not impacted
  const readResult = await conn.read(buf);
  assertEqual(3, readResult.nread);
  assertEqual(1, buf[0]);
  assertEqual(2, buf[1]);
  assertEqual(3, buf[2]);
  // Check write should be closed
  let err;
  try {
    await conn.write(new Uint8Array([1, 2, 3]));
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEqual(err.kind, deno.ErrorKind.BrokenPipe);
  assertEqual(err.name, "BrokenPipe");
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netDoubleCloseWrite() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await deno.dial("tcp", addr);
  conn.closeWrite(); // closing write
  let err;
  try {
    // Duplicated close should throw error
    conn.closeWrite();
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEqual(err.kind, deno.ErrorKind.NotConnected);
  assertEqual(err.name, "NotConnected");
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/
