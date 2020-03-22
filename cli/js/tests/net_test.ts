// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertEquals,
  createResolvable,
  randomPort
} from "./test_util.ts";

unitTest({ perms: { net: true } }, function netTcpListenClose(): void {
  const port = randomPort();
  const listener = Deno.listen({ hostname: "127.0.0.1", port });
  assertEquals(listener.addr.transport, "tcp");
  assertEquals(listener.addr.hostname, "127.0.0.1");
  assertEquals(listener.addr.port, port);
  listener.close();
});

unitTest(
  {
    perms: { net: true },
    // TODO:
    ignore: Deno.build.os === "win"
  },
  function netUdpListenClose(): void {
    const port = randomPort();
    const socket = Deno.listen({
      hostname: "127.0.0.1",
      port,
      transport: "udp"
    });
    assertEquals(socket.addr.transport, "udp");
    assertEquals(socket.addr.hostname, "127.0.0.1");
    assertEquals(socket.addr.port, port);
    socket.close();
  }
);

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  function netUnixListenClose(): void {
    const filePath = Deno.makeTempFileSync();
    const socket = Deno.listen({
      address: filePath,
      transport: "unix"
    });
    assertEquals(socket.addr.transport, "unix");
    assertEquals(socket.addr.address, filePath);
    socket.close();
  }
);

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  function netUnixPacketListenClose(): void {
    const filePath = Deno.makeTempFileSync();
    const socket = Deno.listen({
      address: filePath,
      transport: "unixpacket"
    });
    assertEquals(socket.addr.transport, "unixpacket");
    assertEquals(socket.addr.address, filePath);
    socket.close();
  }
);

unitTest(
  {
    perms: { net: true }
  },
  async function netTcpCloseWhileAccept(): Promise<void> {
    const port = randomPort();
    const listener = Deno.listen({ port });
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
  }
);

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  async function netUnixCloseWhileAccept(): Promise<void> {
    const filePath = await Deno.makeTempFile();
    const listener = Deno.listen({
      address: filePath,
      transport: "unix"
    });
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
  }
);

unitTest(
  { perms: { net: true } },
  async function netTcpConcurrentAccept(): Promise<void> {
    const port = randomPort();
    const listener = Deno.listen({ port });
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
    await Promise.all([p, p1]);
    assertEquals(acceptErrCount, 1);
  }
);

// TODO(jsouto): Uncomment when tokio updates mio to v0.7!
// unitTest(
//   { skip: Deno.build.os === "win", perms: { read: true, write: true } },
//   async function netUnixConcurrentAccept(): Promise<void> {
// if (Deno.build.os === "win") return;
//     const filePath = await Deno.makeTempFile();
//     const listener = Deno.listen({ transport: "unix", address: filePath });
//     let acceptErrCount = 0;
//     const checkErr = (e: Error): void => {
//       if (e.message === "Listener has been closed") {
//         assertEquals(acceptErrCount, 1);
//       } else if (e.message === "Another accept task is ongoing") {
//         acceptErrCount++;
//       } else {
//         throw new Error("Unexpected error message");
//       }
//     };
//     const p = listener.accept().catch(checkErr);
//     const p1 = listener.accept().catch(checkErr);
//     await Promise.race([p, p1]);
//     listener.close();
//     await [p, p1];
//     assertEquals(acceptErrCount, 1);
//   }
// );

unitTest({ perms: { net: true } }, async function netTcpDialListen(): Promise<
  void
> {
  const port = randomPort();
  const listener = Deno.listen({ port });
  listener.accept().then(
    async (conn): Promise<void> => {
      assert(conn.remoteAddr != null);
      assertEquals(conn.localAddr.hostname, "127.0.0.1");
      assertEquals(conn.localAddr.port, port);
      await conn.write(new Uint8Array([1, 2, 3]));
      conn.close();
    }
  );
  const conn = await Deno.connect({ hostname: "127.0.0.1", port });
  assertEquals(conn.remoteAddr.hostname, "127.0.0.1");
  assertEquals(conn.remoteAddr.port, port);
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

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  async function netUnixDialListen(): Promise<void> {
    const filePath = await Deno.makeTempFile();
    const listener = Deno.listen({ address: filePath, transport: "unix" });
    listener.accept().then(
      async (conn): Promise<void> => {
        assert(conn.remoteAddr != null);
        assert(conn.localAddr.transport === "unix");
        assertEquals(conn.localAddr.address, filePath);
        await conn.write(new Uint8Array([1, 2, 3]));
        conn.close();
      }
    );
    const conn = await Deno.connect({ address: filePath, transport: "unix" });
    assert(conn.remoteAddr.transport === "unix");
    assertEquals(conn.remoteAddr.address, filePath);
    assert(conn.remoteAddr != null);
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
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { net: true } },
  async function netUdpSendReceive(): Promise<void> {
    const alicePort = randomPort();
    const alice = Deno.listen({ port: alicePort, transport: "udp" });
    assertEquals(alice.addr.port, alicePort);
    assertEquals(alice.addr.hostname, "0.0.0.0");
    assertEquals(alice.addr.transport, "udp");

    const bobPort = randomPort();
    const bob = Deno.listen({ port: bobPort, transport: "udp" });
    assertEquals(bob.addr.port, bobPort);
    assertEquals(bob.addr.hostname, "0.0.0.0");
    assertEquals(bob.addr.transport, "udp");

    const sent = new Uint8Array([1, 2, 3]);
    await alice.send(sent, bob.addr);

    const [recvd, remote] = await bob.receive();
    assertEquals(remote.port, alicePort);
    assertEquals(recvd.length, 3);
    assertEquals(1, recvd[0]);
    assertEquals(2, recvd[1]);
    assertEquals(3, recvd[2]);
    alice.close();
    bob.close();
  }
);

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  async function netUnixPacketSendReceive(): Promise<void> {
    const filePath = await Deno.makeTempFile();
    const alice = Deno.listen({ address: filePath, transport: "unixpacket" });
    assertEquals(alice.addr.address, filePath);
    assertEquals(alice.addr.transport, "unixpacket");

    const bob = Deno.listen({ address: filePath, transport: "unixpacket" });
    assertEquals(alice.addr.address, filePath);
    assertEquals(alice.addr.transport, "unixpacket");

    const sent = new Uint8Array([1, 2, 3]);
    await alice.send(sent, bob.addr);

    const [recvd, remote] = await bob.receive();
    assertEquals(remote.address, filePath);
    assertEquals(recvd.length, 3);
    assertEquals(1, recvd[0]);
    assertEquals(2, recvd[1]);
    assertEquals(3, recvd[2]);
    alice.close();
    bob.close();
  }
);

unitTest(
  { perms: { net: true } },
  async function netTcpListenCloseWhileIterating(): Promise<void> {
    const port = randomPort();
    const listener = Deno.listen({ port });
    const nextWhileClosing = listener[Symbol.asyncIterator]().next();
    listener.close();
    assertEquals(await nextWhileClosing, { value: undefined, done: true });

    const nextAfterClosing = listener[Symbol.asyncIterator]().next();
    assertEquals(await nextAfterClosing, { value: undefined, done: true });
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { net: true } },
  async function netUdpListenCloseWhileIterating(): Promise<void> {
    const port = randomPort();
    const socket = Deno.listen({ port, transport: "udp" });
    const nextWhileClosing = socket[Symbol.asyncIterator]().next();
    socket.close();
    assertEquals(await nextWhileClosing, { value: undefined, done: true });

    const nextAfterClosing = socket[Symbol.asyncIterator]().next();
    assertEquals(await nextAfterClosing, { value: undefined, done: true });
  }
);

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  async function netUnixListenCloseWhileIterating(): Promise<void> {
    const filePath = Deno.makeTempFileSync();
    const socket = Deno.listen({ address: filePath, transport: "unix" });
    const nextWhileClosing = socket[Symbol.asyncIterator]().next();
    socket.close();
    assertEquals(await nextWhileClosing, { value: undefined, done: true });

    const nextAfterClosing = socket[Symbol.asyncIterator]().next();
    assertEquals(await nextAfterClosing, { value: undefined, done: true });
  }
);

unitTest(
  { skip: Deno.build.os === "win", perms: { read: true, write: true } },
  async function netUnixPacketListenCloseWhileIterating(): Promise<void> {
    const filePath = Deno.makeTempFileSync();
    const socket = Deno.listen({ address: filePath, transport: "unixpacket" });
    const nextWhileClosing = socket[Symbol.asyncIterator]().next();
    socket.close();
    assertEquals(await nextWhileClosing, { value: undefined, done: true });

    const nextAfterClosing = socket[Symbol.asyncIterator]().next();
    assertEquals(await nextAfterClosing, { value: undefined, done: true });
  }
);

unitTest(
  {
    // FIXME(bartlomieju)
    ignore: true,
    perms: { net: true }
  },
  async function netListenAsyncIterator(): Promise<void> {
    const port = randomPort();
    const addr = { hostname: "127.0.0.1", port };
    const listener = Deno.listen(addr);
    const runAsyncIterator = async (): Promise<void> => {
      for await (const conn of listener) {
        await conn.write(new Uint8Array([1, 2, 3]));
        conn.close();
      }
    };
    runAsyncIterator();
    const conn = await Deno.connect(addr);
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
  }
);

unitTest(
  {
    // FIXME(bartlomieju)
    ignore: true,
    perms: { net: true }
  },
  async function netCloseReadSuccess() {
    const port = randomPort();
    const addr = { hostname: "127.0.0.1", port };
    const listener = Deno.listen(addr);
    const closeDeferred = createResolvable();
    const closeReadDeferred = createResolvable();
    listener.accept().then(async conn => {
      await closeReadDeferred;
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
    await closeDeferred;
    listener.close();
    conn.close();
  }
);

unitTest(
  {
    // FIXME(bartlomieju)
    ignore: true,
    perms: { net: true }
  },
  async function netDoubleCloseRead() {
    const port = randomPort();
    const addr = { hostname: "127.0.0.1", port };
    const listener = Deno.listen(addr);
    const closeDeferred = createResolvable();
    listener.accept().then(async conn => {
      await conn.write(new Uint8Array([1, 2, 3]));
      await closeDeferred;
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
  }
);

unitTest(
  {
    // FIXME(bartlomieju)
    ignore: true,
    perms: { net: true }
  },
  async function netCloseWriteSuccess() {
    const port = randomPort();
    const addr = { hostname: "127.0.0.1", port };
    const listener = Deno.listen(addr);
    const closeDeferred = createResolvable();
    listener.accept().then(async conn => {
      await conn.write(new Uint8Array([1, 2, 3]));
      await closeDeferred;
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
  }
);

unitTest(
  {
    // FIXME(bartlomieju)
    ignore: true,
    perms: { net: true }
  },
  async function netDoubleCloseWrite() {
    const port = randomPort();
    const addr = { hostname: "127.0.0.1", port };
    const listener = Deno.listen(addr);
    const closeDeferred = createResolvable();
    listener.accept().then(async conn => {
      await closeDeferred;
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
  }
);

unitTest(
  {
    perms: { net: true }
  },
  async function netHangsOnClose() {
    let acceptedConn: Deno.Conn;
    const resolvable = createResolvable();

    async function iteratorReq(listener: Deno.Listener): Promise<void> {
      const p = new Uint8Array(10);
      const conn = await listener.accept();
      acceptedConn = conn;

      try {
        while (true) {
          const nread = await conn.read(p);
          if (nread === Deno.EOF) {
            break;
          }
          await conn.write(new Uint8Array([1, 2, 3]));
        }
      } catch (err) {
        assert(!!err);
        assert(err instanceof Deno.errors.BadResource);
      }

      resolvable.resolve();
    }
    const port = randomPort();
    const addr = { hostname: "127.0.0.1", port };
    const listener = Deno.listen(addr);
    iteratorReq(listener);
    const conn = await Deno.connect(addr);
    await conn.write(new Uint8Array([1, 2, 3, 4]));
    const buf = new Uint8Array(10);
    await conn.read(buf);
    conn!.close();
    acceptedConn!.close();
    listener.close();
    await resolvable;
  }
);
