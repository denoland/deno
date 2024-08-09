// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const {
  BadResourcePrototype,
  InterruptedPrototype,
  internalRidSymbol,
  createCancelHandle,
} = core;
import {
  op_dns_resolve,
  op_net_accept_tcp,
  op_net_accept_unix,
  op_net_connect_tcp,
  op_net_connect_unix,
  op_net_join_multi_v4_udp,
  op_net_join_multi_v6_udp,
  op_net_leave_multi_v4_udp,
  op_net_leave_multi_v6_udp,
  op_net_listen_tcp,
  op_net_listen_unix,
  op_net_recv_udp,
  op_net_recv_unixpacket,
  op_net_send_udp,
  op_net_send_unixpacket,
  op_net_set_multi_loopback_udp,
  op_net_set_multi_ttl_udp,
  op_set_keepalive,
  op_set_nodelay,
} from "ext:core/ops";
const UDP_DGRAM_MAXSIZE = 65507;

const {
  Error,
  Number,
  ObjectPrototypeIsPrototypeOf,
  ObjectDefineProperty,
  PromiseResolve,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeDelete,
  SetPrototypeForEach,
  SymbolAsyncIterator,
  Symbol,
  TypeError,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

import {
  readableStreamForRidUnrefable,
  readableStreamForRidUnrefableRef,
  readableStreamForRidUnrefableUnref,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import { SymbolDispose } from "ext:deno_web/00_infra.js";

async function write(rid, data) {
  return await core.write(rid, data);
}

function shutdown(rid) {
  return core.shutdown(rid);
}

async function resolveDns(query, recordType, options) {
  let cancelRid;
  let abortHandler;
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = createCancelHandle();
    abortHandler = () => core.tryClose(cancelRid);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    return await op_dns_resolve({
      cancelRid,
      query,
      recordType,
      options,
    });
  } finally {
    if (options?.signal) {
      options.signal[abortSignal.remove](abortHandler);

      // always throw the abort error when aborted
      options.signal.throwIfAborted();
    }
  }
}

class Conn {
  #rid = 0;
  #remoteAddr = null;
  #localAddr = null;
  #unref = false;
  #pendingReadPromises = new SafeSet();

  #preventCloseOnEOF;

  #readable;
  #writable;

  constructor(rid, remoteAddr, localAddr, preventCloseOnEOF) {
    if (internals.future) {
      ObjectDefineProperty(this, "rid", {
        enumerable: false,
        value: undefined,
      });
    }
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
    this.#remoteAddr = remoteAddr;
    this.#localAddr = localAddr;
    this.#preventCloseOnEOF = preventCloseOnEOF;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.Conn.rid",
      new Error().stack,
      "Use `Deno.Conn` instance methods instead.",
    );
    return this.#rid;
  }

  get remoteAddr() {
    return this.#remoteAddr;
  }

  get localAddr() {
    return this.#localAddr;
  }

  write(p) {
    return write(this.#rid, p);
  }

  async read(buffer) {
    if (buffer.length === 0) {
      return 0;
    }
    const promise = core.read(this.#rid, buffer);
    if (this.#unref) core.unrefOpPromise(promise);
    SetPrototypeAdd(this.#pendingReadPromises, promise);
    let nread;
    try {
      nread = await promise;
    } catch (e) {
      throw e;
    } finally {
      SetPrototypeDelete(this.#pendingReadPromises, promise);
    }
    return nread === 0 ? null : nread;
  }

  close() {
    core.close(this.#rid);
  }

  closeWrite() {
    return shutdown(this.#rid);
  }

  get readable() {
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRidUnrefable(
        this.#rid,
        this.#preventCloseOnEOF,
      );
      if (this.#unref) {
        readableStreamForRidUnrefableUnref(this.#readable);
      }
    }
    return this.#readable;
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  ref() {
    this.#unref = false;
    if (this.#readable) {
      readableStreamForRidUnrefableRef(this.#readable);
    }

    SetPrototypeForEach(
      this.#pendingReadPromises,
      (promise) => core.refOpPromise(promise),
    );
  }

  unref() {
    this.#unref = true;
    if (this.#readable) {
      readableStreamForRidUnrefableUnref(this.#readable);
    }
    SetPrototypeForEach(
      this.#pendingReadPromises,
      (promise) => core.unrefOpPromise(promise),
    );
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }
}

class TcpConn extends Conn {
  #rid = 0;

  constructor(rid, remoteAddr, localAddr, preventCloseOnEOF) {
    super(rid, remoteAddr, localAddr, preventCloseOnEOF);
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.TcpConn.rid",
      new Error().stack,
      "Use `Deno.TcpConn` instance methods instead.",
    );
    return this.#rid;
  }

  setNoDelay(noDelay = true) {
    return op_set_nodelay(this.#rid, noDelay);
  }

  setKeepAlive(keepAlive = true) {
    return op_set_keepalive(this.#rid, keepAlive);
  }
}

class UnixConn extends Conn {
  #rid = 0;

  constructor(rid, remoteAddr, localAddr, preventCloseOnEOF) {
    super(rid, remoteAddr, localAddr, preventCloseOnEOF);
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.UnixConn.rid",
      new Error().stack,
      "Use `Deno.UnixConn` instance methods instead.",
    );
    return this.#rid;
  }
}

class Listener {
  #rid = 0;
  #addr = null;
  #unref = false;
  #promise = null;

  constructor(rid, addr) {
    if (internals.future) {
      ObjectDefineProperty(this, "rid", {
        enumerable: false,
        value: undefined,
      });
    }
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
    this.#addr = addr;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.Listener.rid",
      new Error().stack,
      "Use `Deno.Listener` instance methods instead.",
    );
    return this.#rid;
  }

  get addr() {
    return this.#addr;
  }

  async accept({ preventCloseOnEOF = false } = { __proto__: null }) {
    let promise;
    switch (this.addr.transport) {
      case "tcp":
        promise = op_net_accept_tcp(this.#rid);
        break;
      case "unix":
        promise = op_net_accept_unix(this.#rid);
        break;
      default:
        throw new Error(`Unsupported transport: ${this.addr.transport}`);
    }
    this.#promise = promise;
    if (this.#unref) core.unrefOpPromise(promise);
    const { 0: rid, 1: localAddr, 2: remoteAddr } = await promise;
    this.#promise = null;
    if (this.addr.transport == "tcp") {
      localAddr.transport = "tcp";
      remoteAddr.transport = "tcp";
      return new TcpConn(rid, remoteAddr, localAddr, preventCloseOnEOF);
    } else if (this.addr.transport == "unix") {
      return new UnixConn(
        rid,
        { transport: "unix", path: remoteAddr },
        { transport: "unix", path: localAddr },
        preventCloseOnEOF,
      );
    } else {
      throw new Error("unreachable");
    }
  }

  async next(options) {
    let conn;
    try {
      conn = await this.accept(options);
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
    core.close(this.#rid);
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }

  [SymbolAsyncIterator]() {
    return this;
  }

  ref() {
    this.#unref = false;
    if (this.#promise !== null) {
      core.refOpPromise(this.#promise);
    }
  }

  unref() {
    this.#unref = true;
    if (this.#promise !== null) {
      core.unrefOpPromise(this.#promise);
    }
  }
}

class DatagramConn {
  #rid = 0;
  #addr = null;
  #unref = false;
  #promise = null;

  constructor(rid, addr, bufSize = UDP_DGRAM_MAXSIZE) {
    this.#rid = rid;
    this.#addr = addr;
    this.bufSize = bufSize;
  }

  get addr() {
    return this.#addr;
  }

  async joinMulticastV4(addr, multiInterface) {
    await op_net_join_multi_v4_udp(
      this.#rid,
      addr,
      multiInterface,
    );

    return {
      leave: () =>
        op_net_leave_multi_v4_udp(
          this.#rid,
          addr,
          multiInterface,
        ),
      setLoopback: (loopback) =>
        op_net_set_multi_loopback_udp(
          this.#rid,
          true,
          loopback,
        ),
      setTTL: (ttl) =>
        op_net_set_multi_ttl_udp(
          this.#rid,
          ttl,
        ),
    };
  }

  async joinMulticastV6(addr, multiInterface) {
    await op_net_join_multi_v6_udp(
      this.#rid,
      addr,
      multiInterface,
    );

    return {
      leave: () =>
        op_net_leave_multi_v6_udp(
          this.#rid,
          addr,
          multiInterface,
        ),
      setLoopback: (loopback) =>
        op_net_set_multi_loopback_udp(
          this.#rid,
          false,
          loopback,
        ),
    };
  }

  async receive(p) {
    const buf = p || new Uint8Array(this.bufSize);
    let nread;
    let remoteAddr;
    switch (this.addr.transport) {
      case "udp": {
        this.#promise = op_net_recv_udp(
          this.#rid,
          buf,
        );
        if (this.#unref) core.unrefOpPromise(this.#promise);
        ({ 0: nread, 1: remoteAddr } = await this.#promise);
        remoteAddr.transport = "udp";
        break;
      }
      case "unixpacket": {
        let path;
        ({ 0: nread, 1: path } = await op_net_recv_unixpacket(
          this.#rid,
          buf,
        ));
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
        return await op_net_send_udp(
          this.#rid,
          { hostname: opts.hostname ?? "127.0.0.1", port: opts.port },
          p,
        );
      case "unixpacket":
        return await op_net_send_unixpacket(
          this.#rid,
          opts.path,
          p,
        );
      default:
        throw new Error(`Unsupported transport: ${this.addr.transport}`);
    }
  }

  close() {
    core.close(this.#rid);
  }

  ref() {
    this.#unref = false;
    if (this.#promise !== null) {
      core.refOpPromise(this.#promise);
    }
  }

  unref() {
    this.#unref = true;
    if (this.#promise !== null) {
      core.unrefOpPromise(this.#promise);
    }
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

const listenOptionApiName = Symbol("listenOptionApiName");

function listen(args) {
  switch (args.transport ?? "tcp") {
    case "tcp": {
      const { 0: rid, 1: addr } = op_net_listen_tcp({
        hostname: args.hostname ?? "0.0.0.0",
        port: Number(args.port),
      }, args.reusePort);
      addr.transport = "tcp";
      return new Listener(rid, addr);
    }
    case "unix": {
      const { 0: rid, 1: path } = op_net_listen_unix(
        args.path,
        args[listenOptionApiName] ?? "Deno.listen",
      );
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
        const { 0: rid, 1: addr } = udpOpFn(
          {
            hostname: args.hostname ?? "127.0.0.1",
            port: args.port,
          },
          args.reuseAddress ?? false,
          args.loopback ?? false,
        );
        addr.transport = "udp";
        return new DatagramConn(rid, addr);
      }
      case "unixpacket": {
        const { 0: rid, 1: path } = unixOpFn(args.path);
        const addr = {
          transport: "unixpacket",
          path,
        };
        return new DatagramConn(rid, addr);
      }
      default:
        throw new TypeError(`Unsupported transport: '${transport}'`);
    }
  };
}

async function connect(args) {
  switch (args.transport ?? "tcp") {
    case "tcp": {
      const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tcp(
        {
          hostname: args.hostname ?? "127.0.0.1",
          port: args.port,
        },
      );
      localAddr.transport = "tcp";
      remoteAddr.transport = "tcp";
      return new TcpConn(
        rid,
        remoteAddr,
        localAddr,
        args.preventCloseOnEOF ?? false,
      );
    }
    case "unix": {
      const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_unix(
        args.path,
      );
      return new UnixConn(
        rid,
        { transport: "unix", path: remoteAddr },
        { transport: "unix", path: localAddr },
        args.preventCloseOnEOF ?? false,
      );
    }
    default:
      throw new TypeError(`Unsupported transport: '${transport}'`);
  }
}

export {
  Conn,
  connect,
  createListenDatagram,
  listen,
  Listener,
  listenOptionApiName,
  resolveDns,
  shutdown,
  TcpConn,
  UnixConn,
};
