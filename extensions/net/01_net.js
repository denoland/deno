// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { BadResource } = core;
  const {
    PromiseResolve,
    SymbolAsyncIterator,
    Uint8Array,
    TypedArrayPrototypeSubarray,
  } = window.__bootstrap.primordials;

  async function read(
    rid,
    buffer,
  ) {
    if (buffer.length === 0) {
      return 0;
    }
    const nread = await core.opAsync("op_net_read_async", rid, buffer);
    return nread === 0 ? null : nread;
  }

  async function write(rid, data) {
    return await core.opAsync("op_net_write_async", rid, data);
  }

  function shutdown(rid) {
    return core.opAsync("op_net_shutdown", rid);
  }

  function opAccept(rid, transport) {
    return core.opAsync("op_accept", { rid, transport });
  }

  function opListen(args) {
    return core.opSync("op_listen", args);
  }

  function opConnect(args) {
    return core.opAsync("op_connect", args);
  }

  function opReceive(rid, transport, zeroCopy) {
    return core.opAsync(
      "op_datagram_receive",
      { rid, transport },
      zeroCopy,
    );
  }

  function opSend(args, zeroCopy) {
    return core.opAsync("op_datagram_send", args, zeroCopy);
  }

  function resolveDns(query, recordType, options) {
    return core.opAsync("op_dns_resolve", { query, recordType, options });
  }

  class Conn {
    #rid = 0;
    #remoteAddr = null;
    #localAddr = null;
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
  }

  class Listener {
    #rid = 0;
    #addr = null;

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
      const res = await opAccept(this.rid, this.addr.transport);
      return new Conn(res.rid, res.remoteAddr, res.localAddr);
    }

    async next() {
      let conn;
      try {
        conn = await this.accept();
      } catch (error) {
        if (error instanceof BadResource) {
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
      const remote = { hostname: "127.0.0.1", ...addr };

      const args = { ...remote, rid: this.rid };
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
          if (err instanceof BadResource) {
            break;
          }
          throw err;
        }
      }
    }
  }

  function listen({ hostname, ...options }) {
    const res = opListen({
      transport: "tcp",
      hostname: typeof hostname === "undefined" ? "0.0.0.0" : hostname,
      ...options,
    });

    return new Listener(res.rid, res.localAddr);
  }

  async function connect(options) {
    let res;

    if (options.transport === "unix") {
      res = await opConnect(options);
    } else {
      res = await opConnect({
        transport: "tcp",
        hostname: "127.0.0.1",
        ...options,
      });
    }

    return new Conn(res.rid, res.remoteAddr, res.localAddr);
  }

  window.__bootstrap.net = {
    connect,
    Conn,
    opConnect,
    listen,
    opListen,
    Listener,
    shutdown,
    Datagram,
    resolveDns,
  };
})(this);
