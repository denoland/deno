// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { BadResourcePrototype, InterruptedPrototype, ops } = core;
  const {
    readableStreamForRidUnrefable,
    readableStreamForRidUnrefableRef,
    readableStreamForRidUnrefableUnref,
    writableStreamForRid,
  } = window.__bootstrap.streams;
  const {
    Error,
    ObjectPrototypeIsPrototypeOf,
    PromiseResolve,
    SymbolAsyncIterator,
    SymbolFor,
    TypedArrayPrototypeSubarray,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;

  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  async function write(rid, data) {
    return await core.write(rid, data);
  }

  function shutdown(rid) {
    return core.shutdown(rid);
  }

  function resolveDns(query, recordType, options) {
    return core.opAsync("op_dns_resolve", { query, recordType, options });
  }

  class Conn {
    #rid = 0;
    #remoteAddr = null;
    #localAddr = null;
    #unref = false;
    #pendingReadPromiseIds = [];

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

    async read(buffer) {
      if (buffer.length === 0) {
        return 0;
      }
      const promise = core.read(this.rid, buffer);
      const promiseId = promise[promiseIdSymbol];
      if (this.#unref) core.unrefOp(promiseId);
      this.#pendingReadPromiseIds.push(promiseId);
      let nread;
      try {
        nread = await promise;
      } catch (e) {
        throw e;
      } finally {
        this.#pendingReadPromiseIds = this.#pendingReadPromiseIds.filter((id) =>
          id !== promiseId
        );
      }
      return nread === 0 ? null : nread;
    }

    close() {
      core.close(this.rid);
    }

    closeWrite() {
      return shutdown(this.rid);
    }

    get readable() {
      if (this.#readable === undefined) {
        this.#readable = readableStreamForRidUnrefable(this.rid);
        if (this.#unref) {
          readableStreamForRidUnrefableUnref(this.#readable);
        }
      }
      return this.#readable;
    }

    get writable() {
      if (this.#writable === undefined) {
        this.#writable = writableStreamForRid(this.rid);
      }
      return this.#writable;
    }

    ref() {
      this.#unref = false;
      if (this.#readable) {
        readableStreamForRidUnrefableRef(this.#readable);
      }
      this.#pendingReadPromiseIds.forEach((id) => core.refOp(id));
    }

    unref() {
      this.#unref = true;
      if (this.#readable) {
        readableStreamForRidUnrefableUnref(this.#readable);
      }
      this.#pendingReadPromiseIds.forEach((id) => core.unrefOp(id));
    }
  }

  class TcpConn extends Conn {
    setNoDelay(noDelay = true) {
      return ops.op_set_nodelay(this.rid, noDelay);
    }

    setKeepAlive(keepAlive = true) {
      return ops.op_set_keepalive(this.rid, keepAlive);
    }
  }

  class UnixConn extends Conn {}

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

    async accept() {
      let promise;
      switch (this.addr.transport) {
        case "tcp":
          promise = core.opAsync("op_net_accept_tcp", this.rid);
          break;
        case "unix":
          promise = core.opAsync("op_net_accept_unix", this.rid);
          break;
        default:
          throw new Error(`Unsupported transport: ${this.addr.transport}`);
      }
      this.#promiseId = promise[promiseIdSymbol];
      if (this.#unref) core.unrefOp(this.#promiseId);
      const [rid, localAddr, remoteAddr] = await promise;
      this.#promiseId = null;
      if (this.addr.transport == "tcp") {
        localAddr.transport = "tcp";
        remoteAddr.transport = "tcp";
        return new TcpConn(rid, remoteAddr, localAddr);
      } else if (this.addr.transport == "unix") {
        return new UnixConn(
          rid,
          { transport: "unix", path: remoteAddr },
          { transport: "unix", path: localAddr },
        );
      } else {
        throw new Error("unreachable");
      }
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

    ref() {
      this.#unref = false;
      if (typeof this.#promiseId === "number") {
        core.refOp(this.#promiseId);
      }
    }

    unref() {
      this.#unref = true;
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
      let nread;
      let remoteAddr;
      switch (this.addr.transport) {
        case "udp": {
          [nread, remoteAddr] = await core.opAsync(
            "op_net_recv_udp",
            this.rid,
            buf,
          );
          remoteAddr.transport = "udp";
          break;
        }
        case "unixpacket": {
          let path;
          [nread, path] = await core.opAsync(
            "op_net_recv_unixpacket",
            this.rid,
            buf,
          );
          remoteAddr = { transport: "unixpacket", path };
          break;
        }
        default:
          throw new Error(`Unsupported transport: ${this.addr.transport}`);
      }
      const sub = TypedArrayPrototypeSubarray(buf, 0, nread);
      return [sub, remoteAddr];
    }

    async send(p, opts) {
      switch (this.addr.transport) {
        case "udp":
          return await core.opAsync(
            "op_net_send_udp",
            this.rid,
            { hostname: opts.hostname ?? "127.0.0.1", port: opts.port },
            p,
          );
        case "unixpacket":
          return await core.opAsync(
            "op_net_send_unixpacket",
            this.rid,
            opts.path,
            p,
          );
        default:
          throw new Error(`Unsupported transport: ${this.addr.transport}`);
      }
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

  function listen(args) {
    switch (args.transport ?? "tcp") {
      case "tcp": {
        const [rid, addr] = ops.op_net_listen_tcp({
          hostname: args.hostname ?? "0.0.0.0",
          port: args.port,
        }, args.reusePort);
        addr.transport = "tcp";
        return new Listener(rid, addr);
      }
      case "unix": {
        const [rid, path] = ops.op_net_listen_unix(args.path);
        const addr = {
          transport: "unix",
          path,
        };
        return new Listener(rid, addr);
      }
      default:
        throw new TypeError(`Unsupported transport: '${transport}'`);
    }
  }

  function createListenDatagram(udpOpFn, unixOpFn) {
    return function listenDatagram(args) {
      switch (args.transport) {
        case "udp": {
          const [rid, addr] = udpOpFn(
            {
              hostname: args.hostname ?? "127.0.0.1",
              port: args.port,
            },
            args.reuseAddress ?? false,
          );
          addr.transport = "udp";
          return new Datagram(rid, addr);
        }
        case "unixpacket": {
          const [rid, path] = unixOpFn(args.path);
          const addr = {
            transport: "unixpacket",
            path,
          };
          return new Datagram(rid, addr);
        }
        default:
          throw new TypeError(`Unsupported transport: '${transport}'`);
      }
    };
  }

  async function connect(args) {
    switch (args.transport ?? "tcp") {
      case "tcp": {
        const [rid, localAddr, remoteAddr] = await core.opAsync(
          "op_net_connect_tcp",
          {
            hostname: args.hostname ?? "127.0.0.1",
            port: args.port,
          },
        );
        localAddr.transport = "tcp";
        remoteAddr.transport = "tcp";
        return new TcpConn(rid, remoteAddr, localAddr);
      }
      case "unix": {
        const [rid, localAddr, remoteAddr] = await core.opAsync(
          "op_net_connect_unix",
          args.path,
        );
        return new UnixConn(
          rid,
          { transport: "unix", path: remoteAddr },
          { transport: "unix", path: localAddr },
        );
      }
      default:
        throw new TypeError(`Unsupported transport: '${transport}'`);
    }
  }

  function setup(unstable) {
    if (!unstable) {
      delete Listener.prototype.ref;
      delete Listener.prototype.unref;
    }
  }

  window.__bootstrap.net = {
    setup,
    connect,
    Conn,
    TcpConn,
    UnixConn,
    listen,
    createListenDatagram,
    Listener,
    shutdown,
    Datagram,
    resolveDns,
  };
})(this);
