// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ net: true }, function netListenClose(): void {
  const listener = Deno.listen("tcp", "127.0.0.1:4500");
  listener.close();
});

testPerm({ net: true }, async function netCloseWhileAccept(): Promise<void> {
  const listener = Deno.listen("tcp", ":4501");
  const p = listener.accept();
  listener.close();
  let err;
  try {
    await p;
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEquals(err.kind, Deno.ErrorKind.Other);
  assertEquals(err.message, "Listener has been closed");
});

testPerm({ net: true }, async function netConcurrentAccept(): Promise<void> {
  const listener = Deno.listen("tcp", ":4502");
  let acceptErrCount = 0;
  const checkErr = (e): void => {
    assertEquals(e.kind, Deno.ErrorKind.Other);
    if (e.message === "Listener has been closed") {
      assertEquals(acceptErrCount, 1);
    } else if (e.message === "Another accept task is ongoing") {
      acceptErrCount++;
    } else {
      throw new Error("Unexpected error message");
    }
  };
  const p = listener.accept().catch(checkErr);
  const p1 = listener.accept().catch(checkErr);
  await Promise.race([p, p1]);
  listener.close();
  await [p, p1];
  assertEquals(acceptErrCount, 1);
});

testPerm({ net: true }, async function netDialListen(): Promise<void> {
  const listener = Deno.listen("tcp", ":4500");
  listener.accept().then(
    async (conn): Promise<void> => {
      await conn.write(new Uint8Array([1, 2, 3]));
      conn.close();
    }
  );
  const conn = await Deno.dial("tcp", "127.0.0.1:4500");
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEquals(3, readResult.nread);
  assertEquals(1, buf[0]);
  assertEquals(2, buf[1]);
  assertEquals(3, buf[2]);
  assert(conn.rid > 0);

  // TODO Currently ReadResult does not properly transmit EOF in the same call.
  // it requires a second call to get the EOF. Either ReadResult to be an
  // integer in which 0 signifies EOF or the handler should be modified so that
  // EOF is properly transmitted.
  assertEquals(false, readResult.eof);

  const readResult2 = await conn.read(buf);
  assertEquals(true, readResult2.eof);

  listener.close();
  conn.close();
});

/* TODO(ry) Re-enable this test.
testPerm({ net: true }, async function netListenAsyncIterator(): Promise<void> {
  const listener = Deno.listen("tcp", ":4500");
  const runAsyncIterator = async (): Promise<void> => {
    for await (let conn of listener) {
      await conn.write(new Uint8Array([1, 2, 3]));
      conn.close();
    }
  };
  runAsyncIterator();
  const conn = await Deno.dial("tcp", "127.0.0.1:4500");
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEquals(3, readResult.nread);
  assertEquals(1, buf[0]);
  assertEquals(2, buf[1]);
  assertEquals(3, buf[2]);
  assert(conn.rid > 0);

  // TODO Currently ReadResult does not properly transmit EOF in the same call.
  // it requires a second call to get the EOF. Either ReadResult to be an
  // integer in which 0 signifies EOF or the handler should be modified so that
  // EOF is properly transmitted.
  assertEquals(false, readResult.eof);

  const readResult2 = await conn.read(buf);
  assertEquals(true, readResult2.eof);

  listener.close();
  conn.close();
});
 */

/* TODO Fix broken test.
testPerm({ net: true }, async function netCloseReadSuccess() {
  const addr = "127.0.0.1:4500";
  const listener = Deno.listen("tcp", addr);
  const closeDeferred = deferred();
  const closeReadDeferred = deferred();
  listener.accept().then(async conn => {
    await closeReadDeferred.promise;
    await conn.write(new Uint8Array([1, 2, 3]));
    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assertEquals(3, readResult.nread);
    assertEquals(4, buf[0]);
    assertEquals(5, buf[1]);
    assertEquals(6, buf[2]);
    conn.close();
    closeDeferred.resolve();
  });
  const conn = await Deno.dial("tcp", addr);
  conn.closeRead(); // closing read
  closeReadDeferred.resolve();
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEquals(0, readResult.nread); // No error, read nothing
  assertEquals(true, readResult.eof); // with immediate EOF
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
  const listener = Deno.listen("tcp", addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await Deno.dial("tcp", addr);
  conn.closeRead(); // closing read
  let err;
  try {
    // Duplicated close should throw error
    conn.closeRead();
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEquals(err.kind, Deno.ErrorKind.NotConnected);
  assertEquals(err.name, "NotConnected");
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netCloseWriteSuccess() {
  const addr = "127.0.0.1:4500";
  const listener = Deno.listen("tcp", addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await Deno.dial("tcp", addr);
  conn.closeWrite(); // closing write
  const buf = new Uint8Array(1024);
  // Check read not impacted
  const readResult = await conn.read(buf);
  assertEquals(3, readResult.nread);
  assertEquals(1, buf[0]);
  assertEquals(2, buf[1]);
  assertEquals(3, buf[2]);
  // Check write should be closed
  let err;
  try {
    await conn.write(new Uint8Array([1, 2, 3]));
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEquals(err.kind, Deno.ErrorKind.BrokenPipe);
  assertEquals(err.name, "BrokenPipe");
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netDoubleCloseWrite() {
  const addr = "127.0.0.1:4500";
  const listener = Deno.listen("tcp", addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await Deno.dial("tcp", addr);
  conn.closeWrite(); // closing write
  let err;
  try {
    // Duplicated close should throw error
    conn.closeWrite();
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEquals(err.kind, Deno.ErrorKind.NotConnected);
  assertEquals(err.name, "NotConnected");
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/
