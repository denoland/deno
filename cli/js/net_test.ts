// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ net: true }, function netTcpListenClose(): void {
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 4500 });
  assertEquals(listener.addr.transport, "tcp");
  assertEquals(listener.addr.hostname, "127.0.0.1");
  assertEquals(listener.addr.port, 4500);
  listener.close();
});

testPerm({ net: true }, function netUdpListenClose(): void {
  if (Deno.build.os === "win") return; // TODO

  const socket = Deno.listen({
    hostname: "127.0.0.1",
    port: 4500,
    transport: "udp"
  });
  assertEquals(socket.addr.transport, "udp");
  assertEquals(socket.addr.hostname, "127.0.0.1");
  assertEquals(socket.addr.port, 4500);
  socket.close();
});

testPerm({ net: true }, async function netTcpCloseWhileAccept(): Promise<void> {
  const listener = Deno.listen({ port: 4501 });
  const p = listener.accept();
  listener.close();
  let err;
  try {
    await p;
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assert(err instanceof Error);
  assertEquals(err.message, "Listener has been closed");
});

testPerm({ net: true }, async function netTcpConcurrentAccept(): Promise<void> {
  const listener = Deno.listen({ port: 4502 });
  let acceptErrCount = 0;
  const checkErr = (e: Error): void => {
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

testPerm({ net: true }, async function netTcpDialListen(): Promise<void> {
  const listener = Deno.listen({ port: 4500 });
  listener.accept().then(
    async (conn): Promise<void> => {
      assert(conn.remoteAddr != null);
      assertEquals(conn.localAddr.hostname, "127.0.0.1");
      assertEquals(conn.localAddr.port, 4500);
      await conn.write(new Uint8Array([1, 2, 3]));
      conn.close();
    }
  );
  const conn = await Deno.connect({ hostname: "127.0.0.1", port: 4500 });
  assertEquals(conn.remoteAddr.hostname, "127.0.0.1");
  assertEquals(conn.remoteAddr.port, 4500);
  assert(conn.localAddr != null);
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEquals(3, readResult);
  assertEquals(1, buf[0]);
  assertEquals(2, buf[1]);
  assertEquals(3, buf[2]);
  assert(conn.rid > 0);

  assert(readResult !== Deno.EOF);

  const readResult2 = await conn.read(buf);
  assertEquals(Deno.EOF, readResult2);

  listener.close();
  conn.close();
});

testPerm({ net: true }, async function netUdpSendReceive(): Promise<void> {
  if (Deno.build.os === "win") return; // TODO

  const alice = Deno.listen({ port: 4500, transport: "udp" });
  assertEquals(alice.addr.port, 4500);
  assertEquals(alice.addr.hostname, "0.0.0.0");
  assertEquals(alice.addr.transport, "udp");

  const bob = Deno.listen({ port: 4501, transport: "udp" });
  assertEquals(bob.addr.port, 4501);
  assertEquals(bob.addr.hostname, "0.0.0.0");
  assertEquals(bob.addr.transport, "udp");

  const sent = new Uint8Array([1, 2, 3]);
  await alice.send(sent, bob.addr);

  const [recvd, remote] = await bob.receive();
  assertEquals(remote.port, 4500);
  assertEquals(recvd.length, 3);
  assertEquals(1, recvd[0]);
  assertEquals(2, recvd[1]);
  assertEquals(3, recvd[2]);
});

testPerm(
  { net: true },
  async function netTcpListenCloseWhileIterating(): Promise<void> {
    const listener = Deno.listen({ port: 8000 });
    const nextWhileClosing = listener[Symbol.asyncIterator]().next();
    listener.close();
    assertEquals(await nextWhileClosing, { value: undefined, done: true });

    const nextAfterClosing = listener[Symbol.asyncIterator]().next();
    assertEquals(await nextAfterClosing, { value: undefined, done: true });
  }
);

testPerm(
  { net: true },
  async function netUdpListenCloseWhileIterating(): Promise<void> {
    if (Deno.build.os === "win") return; // TODO

    const socket = Deno.listen({ port: 8000, transport: "udp" });
    const nextWhileClosing = socket[Symbol.asyncIterator]().next();
    socket.close();
    assertEquals(await nextWhileClosing, { value: undefined, done: true });

    const nextAfterClosing = socket[Symbol.asyncIterator]().next();
    assertEquals(await nextAfterClosing, { value: undefined, done: true });
  }
);

/* TODO(ry) Re-enable this test.
testPerm({ net: true }, async function netListenAsyncIterator(): Promise<void> {
  const listener = Deno.listen(":4500");
  const runAsyncIterator = async (): Promise<void> => {
    for await (let conn of listener) {
      await conn.write(new Uint8Array([1, 2, 3]));
      conn.close();
    }
  };
  runAsyncIterator();
  const conn = await Deno.connect("127.0.0.1:4500");
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEquals(3, readResult);
  assertEquals(1, buf[0]);
  assertEquals(2, buf[1]);
  assertEquals(3, buf[2]);
  assert(conn.rid > 0);

  assert(readResult !== Deno.EOF);

  const readResult2 = await conn.read(buf);
  assertEquals(Deno.EOF, readResult2);

  listener.close();
  conn.close();
});
 */

/* TODO Fix broken test.
testPerm({ net: true }, async function netCloseReadSuccess() {
  const addr = "127.0.0.1:4500";
  const listener = Deno.listen(addr);
  const closeDeferred = deferred();
  const closeReadDeferred = deferred();
  listener.accept().then(async conn => {
    await closeReadDeferred.promise;
    await conn.write(new Uint8Array([1, 2, 3]));
    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assertEquals(3, readResult);
    assertEquals(4, buf[0]);
    assertEquals(5, buf[1]);
    assertEquals(6, buf[2]);
    conn.close();
    closeDeferred.resolve();
  });
  const conn = await Deno.connect(addr);
  conn.closeRead(); // closing read
  closeReadDeferred.resolve();
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEquals(Deno.EOF, readResult); // with immediate EOF
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
  const listener = Deno.listen(addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await Deno.connect(addr);
  conn.closeRead(); // closing read
  let err;
  try {
    // Duplicated close should throw error
    conn.closeRead();
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assert(err instanceof Deno.errors.NotConnected);
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netCloseWriteSuccess() {
  const addr = "127.0.0.1:4500";
  const listener = Deno.listen(addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await Deno.connect(addr);
  conn.closeWrite(); // closing write
  const buf = new Uint8Array(1024);
  // Check read not impacted
  const readResult = await conn.read(buf);
  assertEquals(3, readResult);
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
  assert(err instanceof Deno.errors.BrokenPipe);
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/

/* TODO Fix broken test.
testPerm({ net: true }, async function netDoubleCloseWrite() {
  const addr = "127.0.0.1:4500";
  const listener = Deno.listen(addr);
  const closeDeferred = deferred();
  listener.accept().then(async conn => {
    await closeDeferred.promise;
    conn.close();
  });
  const conn = await Deno.connect(addr);
  conn.closeWrite(); // closing write
  let err;
  try {
    // Duplicated close should throw error
    conn.closeWrite();
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assert(err instanceof Deno.errors.NotConnected);
  closeDeferred.resolve();
  listener.close();
  conn.close();
});
*/
