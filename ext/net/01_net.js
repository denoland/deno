// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { BadResourcePrototype, InterruptedPrototype, ops } = core;
  const { WritableStream, readableStreamForRid } = window.__bootstrap.streams;
  const {
    Error,
    ObjectPrototypeIsPrototypeOf,
    PromiseResolve,
    Symbol,
    SymbolAsyncIterator,
    SymbolFor,
    TypedArrayPrototypeSubarray,
    Uint8Array,
  } = window.__bootstrap.primordials;

  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  async function read(
    rid,
    buffer,
  ) {
    if (buffer.length === 0) {
      return 0;
    }
    const nread = await core.read(rid, buffer);
    return nread === 0 ? null : nread;
  }

  async function write(rid, data) {
    return await core.write(rid, data);
  }

  function shutdown(rid) {
    return core.shutdown(rid);
  }

  function opAccept(rid, transport) {
    return core.opAsync("op_net_accept", { rid, transport });
  }

  function opListen(args) {
    return ops.op_net_listen(args);
  }

  function opConnect(args) {
    return core.opAsync("op_net_connect", args);
  }

  function opReceive(rid, transport, zeroCopy) {
    return core.opAsync(
      "op_dgram_recv",
      { rid, transport },
      zeroCopy,
    );
  }

  function opSend(args, zeroCopy) {
    return core.opAsync("op_dgram_send", args, zeroCopy);
  }

  function resolveDns(query, recordType, options) {
    return core.opAsync("op_dns_resolve", { query, recordType, options });
  }

  function tryClose(rid) {
    try {
      core.close(rid);
    } catch {
      // Ignore errors
    }
  }

  function writableStreamForRid(rid) {
    return new WritableStream({
      async write(chunk, controller) {
        try {
          let nwritten = 0;
          while (nwritten < chunk.length) {
            nwritten += await write(
              rid,
              TypedArrayPrototypeSubarray(chunk, nwritten),
            );
          }
        } catch (e) {
          controller.error(e);
          tryClose(rid);
        }
      },
      close() {
        tryClose(rid);
      },
      abort() {
        tryClose(rid);
      },
    });
  }

  class Conn {
    #rid = 0;
    #remoteAddr = null;
    #localAddr = null;

    #readable;
    #writable;

    constructor(rid, remoteAddr, localAddr) {
      this.#rid = rid;
      this.#remoteAddr = remoteAddr;
      this.#localAddr = localAddr;
    }

    get rid() {
      return this.#rid;
    }

    get remoteAddr() {
      return this.#remoteAddr;
    }

    get localAddr() {
      return this.#localAddr;
    }

    write(p) {
      return write(this.rid, p);
    }

    read(p) {
      return read(this.rid, p);
    }

    close() {
      core.close(this.rid);
    }

    closeWrite() {
      return shutdown(this.rid);
    }

    get readable() {
      if (this.#readable === undefined) {
        this.#readable = readableStreamForRid(this.rid);
      }
      return this.#readable;
    }

    get writable() {
      if (this.#writable === undefined) {
        this.#writable = writableStreamForRid(this.rid);
      }
      return this.#writable;
    }
  }

  class TcpConn extends Conn {
    setNoDelay(nodelay = true) {
      return ops.op_set_nodelay(this.rid, nodelay);
    }

    setKeepAlive(keepalive = true) {
      return ops.op_set_keepalive(this.rid, keepalive);
    }
  }

  class UnixConn extends Conn {}

  // Use symbols for method names to hide these in stable API.
  // TODO(kt3k): Remove these symbols when ref/unref become stable.
  const listenerRef = Symbol("listenerRef");
  const listenerUnref = Symbol("listenerUnref");

  class Listener {
    #rid = 0;
    #addr = null;
    #unref = false;
    #promiseId = null;

    constructor(rid, addr) {
      this.#rid = rid;
      this.#addr = addr;
    }

    get rid() {
      return this.#rid;
    }

    get addr() {
      return this.#addr;
    }

    accept() {
      const promise = opAccept(this.rid, this.addr.transport);
      this.#promiseId = promise[promiseIdSymbol];
      if (this.#unref) {
        this.#unrefOpAccept();
      }
      return promise.then((res) => {
        if (this.addr.transport == "tcp") {
          return new TcpConn(res.rid, res.remoteAddr, res.localAddr);
        } else if (this.addr.transport == "unix") {
          return new UnixConn(res.rid, res.remoteAddr, res.localAddr);
        } else {
          throw new Error("unreachable");
        }
      });
    }

    async next() {
      let conn;
      try {
        conn = await this.accept();
      } catch (error) {
        if (
          ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) ||
          ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error)
        ) {
          return { value: undefined, done: true };
        }
        throw error;
      }
      return { value: conn, done: false };
    }

    return(value) {
      this.close();
      return PromiseResolve({ value, done: true });
    }

    close() {
      core.close(this.rid);
    }

    [SymbolAsyncIterator]() {
      return this;
    }

    [listenerRef]() {
      this.#unref = false;
      this.#refOpAccept();
    }

    [listenerUnref]() {
      this.#unref = true;
      this.#unrefOpAccept();
    }

    #refOpAccept() {
      if (typeof this.#promiseId === "number") {
        core.refOp(this.#promiseId);
      }
    }
    #unrefOpAccept() {
      if (typeof this.#promiseId === "number") {
        core.unrefOp(this.#promiseId);
      }
    }
  }

  class Datagram {
    #rid = 0;
    #addr = null;

    constructor(rid, addr, bufSize = 1024) {
      this.#rid = rid;
      this.#addr = addr;
      this.bufSize = bufSize;
    }

    get rid() {
      return this.#rid;
    }

    get addr() {
      return this.#addr;
    }

    async receive(p) {
      const buf = p || new Uint8Array(this.bufSize);
      const { size, remoteAddr } = await opReceive(
        this.rid,
        this.addr.transport,
        buf,
      );
      const sub = TypedArrayPrototypeSubarray(buf, 0, size);
      return [sub, remoteAddr];
    }

    send(p, addr) {
      const args = { hostname: "127.0.0.1", ...addr, rid: this.rid };
      return opSend(args, p);
    }

    close() {
      core.close(this.rid);
    }

    async *[SymbolAsyncIterator]() {
      while (true) {
        try {
          yield await this.receive();
        } catch (err) {
          if (
            ObjectPrototypeIsPrototypeOf(BadResourcePrototype, err) ||
            ObjectPrototypeIsPrototypeOf(InterruptedPrototype, err)
          ) {
            break;
          }
          throw err;
        }
      }
    }
  }

  function listen({ hostname, ...options }, constructor = Listener) {
    const res = opListen({
      transport: "tcp",
      hostname: typeof hostname === "undefined" ? "0.0.0.0" : hostname,
      ...options,
    });

    return new constructor(res.rid, res.localAddr);
  }

  async function connect(options) {
    if (options.transport === "unix") {
      const res = await opConnect(options);
      return new UnixConn(res.rid, res.remoteAddr, res.localAddr);
    }

    const res = await opConnect({
      transport: "tcp",
      hostname: "127.0.0.1",
      ...options,
    });
    return new TcpConn(res.rid, res.remoteAddr, res.localAddr);
  }

  window.__bootstrap.net = {
    connect,
    Conn,
    TcpConn,
    UnixConn,
    opConnect,
    listen,
    listenerRef,
    listenerUnref,
    opListen,
    Listener,
    shutdown,
    Datagram,
    resolveDns,
  };
  window.__bootstrap.streamUtils = {
    readableStreamForRid,
    writableStreamForRid,
  };
})(this);
