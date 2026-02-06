// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// This module ports:
// - https://github.com/nodejs/node/blob/master/src/tcp_wrap.cc
// - https://github.com/nodejs/node/blob/master/src/tcp_wrap.h

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { op_net_connect_tcp, TCP as NativeTCP } from "ext:core/ops";
import { TcpConn } from "ext:deno_net/01_net.js";
import { core, primordials } from "ext:core/mod.js";
const { internalFdSymbol } = core;
const { Error } = primordials;
import { notImplemented } from "ext:deno_node/_utils.ts";
import { ConnectionWrap } from "ext:deno_node/internal_binding/connection_wrap.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";
import {
  kArrayBufferOffset,
  kBytesWritten,
  kReadBytesOrError,
  kStreamBaseField,
  LibuvStreamWrap,
  ShutdownWrap,
  streamBaseState,
  WriteWrap,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
import { ownerSymbol } from "ext:deno_node/internal_binding/symbols.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { delay } from "ext:deno_node/_util/async.ts";
import { getIPFamily } from "ext:deno_node/internal/net.ts";
import {
  ceilPowOf2,
  INITIAL_ACCEPT_BACKOFF_DELAY,
  MAX_ACCEPT_BACKOFF_DELAY,
} from "ext:deno_node/internal_binding/_listen.ts";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import { Buffer } from "node:buffer";

/** The type of TCP socket. */
enum socketType {
  SOCKET,
  SERVER,
}

interface AddressInfo {
  address: string;
  family?: string;
  port: number;
}

export class TCPConnectWrap extends AsyncWrap {
  oncomplete!: (
    status: number,
    handle: ConnectionWrap,
    req: TCPConnectWrap,
    readable: boolean,
    writeable: boolean,
  ) => void;
  address!: string;
  port!: number;
  localAddress!: string;
  localPort!: number;

  constructor() {
    super(providerType.TCPCONNECTWRAP);
  }
}

export enum constants {
  SOCKET = socketType.SOCKET,
  SERVER = socketType.SERVER,
  UV_TCP_IPV6ONLY,
}

export class TCP extends ConnectionWrap {
  [ownerSymbol]: unknown = null;
  override reading = false;

  #address?: string;
  #port?: number;

  #remoteAddress?: string;
  #remoteFamily?: string;
  #remotePort?: number;

  #backlog?: number;
  #listener!: Deno.Listener;
  #connections = 0;

  #closed = false;
  #acceptBackoffDelay?: number;

  #netPermToken?: object | undefined;

  // Native libuv TCP handle
  #native: any;
  // True when using the native libuv path (bind/listen/accept)
  #useNative = false;

  /**
   * Creates a new TCP class instance.
   * @param type The socket type.
   * @param conn Optional connection object to wrap.
   */
  constructor(type: number, conn?: Deno.Conn) {
    let provider: providerType;

    switch (type) {
      case socketType.SOCKET: {
        provider = providerType.TCPWRAP;

        break;
      }
      case socketType.SERVER: {
        provider = providerType.TCPSERVERWRAP;

        break;
      }
      default: {
        throw new Error("Unreachable code");
      }
    }

    super(provider, conn);

    // Create native libuv TCP handle
    this.#native = new NativeTCP(type);
    this.#native.setOwner();

    // TODO(cmorten): the handling of new connections and construction feels
    // a little off. Suspect duplicating in some fashion.
    if (conn && provider === providerType.TCPWRAP) {
      const localAddr = conn.localAddr as Deno.NetAddr;
      this.#address = localAddr.hostname;
      this.#port = localAddr.port;

      const remoteAddr = conn.remoteAddr as Deno.NetAddr;
      this.#remoteAddress = remoteAddr.hostname;
      this.#remotePort = remoteAddr.port;
      this.#remoteFamily = getIPFamily(remoteAddr.hostname);
    }
  }

  get fd() {
    return this[kStreamBaseField]?.[internalFdSymbol];
  }

  /**
   * Opens a file descriptor.
   * @param fd The file descriptor to open.
   * @return An error status code.
   */
  open(_fd: number): number {
    // REF: https://github.com/denoland/deno/issues/6529
    notImplemented("TCP.prototype.open");
  }

  /**
   * Bind to an IPv4 address.
   * @param address The hostname to bind to.
   * @param port The port to bind to
   * @return An error status code.
   */
  bind(address: string, port: number): number {
    this.#address = address;
    this.#port = port;
    this.#useNative = true;
    return this.#native.bind(address, port);
  }

  /**
   * Bind to an IPv6 address.
   * @param address The hostname to bind to.
   * @param port The port to bind to
   * @return An error status code.
   */
  bind6(address: string, port: number, _flags: number): number {
    this.#address = address;
    this.#port = port;
    this.#useNative = true;
    return this.#native.bind6(address, port);
  }

  /**
   * Connect to an IPv4 address.
   * @param req A TCPConnectWrap instance.
   * @param address The hostname to connect to.
   * @param port The port to connect to.
   * @return An error status code.
   */
  connect(req: TCPConnectWrap, address: string, port: number): number {
    return this.#connect(req, address, port);
  }

  /**
   * Connect to an IPv6 address.
   * @param req A TCPConnectWrap instance.
   * @param address The hostname to connect to.
   * @param port The port to connect to.
   * @return An error status code.
   */
  connect6(req: TCPConnectWrap, address: string, port: number): number {
    return this.#connect(req, address, port);
  }

  /**
   * Listen for new connections.
   * @param backlog The maximum length of the queue of pending connections.
   * @return An error status code.
   */
  listen(backlog: number): number {
    if (!this.#useNative) {
      return this.#listenLegacy(backlog);
    }

    this.#backlog = ceilPowOf2(backlog + 1);

    const self = this;
    this.#native.onconnection = function (status: number) {
      if (status !== 0) {
        try {
          self.onconnection!(status, undefined);
        } catch {
          // swallow callback errors.
        }
        return;
      }

      self.#connections++;
      const clientHandle = new TCP(socketType.SOCKET);
      clientHandle.#useNative = true;
      self.#native.accept(clientHandle.#native);
      clientHandle.#native.setOwner();

      try {
        self.onconnection!(0, clientHandle);
      } catch {
        // swallow callback errors.
      }
    };

    return this.#native.listen(this.#backlog);
  }

  override readStart(): number {
    if (!this.#useNative || !this.#native) {
      return super.readStart();
    }

    this.reading = true;
    const self = this;
    this.#native.onread = function (
      nread: number,
      buf: Uint8Array | undefined,
    ) {
      streamBaseState[kReadBytesOrError] = nread;
      if (nread > 0) {
        self.bytesRead += nread;
      }
      streamBaseState[kArrayBufferOffset] = 0;

      try {
        self.onread!(buf!, nread);
      } catch {
        // swallow callback errors.
      }

      if (nread < 0) {
        self.reading = false;
      }
    };

    return this.#native.readStart();
  }

  override readStop(): number {
    if (!this.#useNative || !this.#native) {
      return super.readStop();
    }

    this.reading = false;
    return this.#native.readStop();
  }

  override writeBuffer(
    req: WriteWrap<LibuvStreamWrap>,
    data: Uint8Array,
  ): number {
    if (!this.#useNative || !this.#native) {
      return super.writeBuffer(req, data);
    }

    const ret = this.#native.writeBuffer(data);
    streamBaseState[kBytesWritten] = data.byteLength;
    this.bytesWritten += data.byteLength;

    // Simulate async completion like Node.js
    queueMicrotask(() => {
      try {
        req.oncomplete(ret === 0 ? 0 : codeMap.get("UNKNOWN")!);
      } catch {
        // swallow callback errors.
      }
    });

    return 0;
  }

  override writev(
    req: WriteWrap<LibuvStreamWrap>,
    chunks: Buffer[] | (string | Buffer)[],
    allBuffers: boolean,
  ): number {
    if (!this.#useNative) {
      return super.writev(req, chunks, allBuffers);
    }

    // For native path, concat all chunks and write as single buffer
    const count = allBuffers ? chunks.length : chunks.length >> 1;
    const buffers: Buffer[] = new Array(count);

    if (!allBuffers) {
      for (let i = 0; i < count; i++) {
        const chunk = chunks[i * 2];
        if (Buffer.isBuffer(chunk)) {
          buffers[i] = chunk;
        } else {
          const encoding: string = chunks[i * 2 + 1] as string;
          buffers[i] = Buffer.from(chunk as string, encoding);
        }
      }
    } else {
      for (let i = 0; i < count; i++) {
        buffers[i] = chunks[i] as Buffer;
      }
    }

    // deno-lint-ignore prefer-primordials
    return this.writeBuffer(req, Buffer.concat(buffers));
  }

  override shutdown(req: ShutdownWrap<LibuvStreamWrap>): number {
    if (!this.#useNative) {
      return super.shutdown(req);
    }

    // For native libuv, shutdown just signals write-side done.
    // Don't close the handle - HandleWrap.close() does that.
    try {
      req.oncomplete(0);
    } catch {
      // swallow callback errors.
    }

    return 0;
  }

  override ref() {
    if (this.#listener) {
      this.#listener.ref();
    }

    if (this[kStreamBaseField]) {
      this[kStreamBaseField].ref();
    }
  }

  override unref() {
    if (this.#listener) {
      this.#listener.unref();
    }

    if (this[kStreamBaseField]) {
      this[kStreamBaseField].unref();
    }
  }

  /**
   * Populates the provided object with local address entries.
   * @param sockname An object to add the local address entries to.
   * @return An error status code.
   */
  getsockname(sockname: Record<string, never> | AddressInfo): number {
    if (
      typeof this.#address === "undefined" ||
      typeof this.#port === "undefined"
    ) {
      return codeMap.get("EADDRNOTAVAIL")!;
    }

    sockname.address = this.#address;
    sockname.port = this.#port;
    sockname.family = getIPFamily(this.#address);

    return 0;
  }

  /**
   * Populates the provided object with remote address entries.
   * @param peername An object to add the remote address entries to.
   * @return An error status code.
   */
  getpeername(peername: Record<string, never> | AddressInfo): number {
    if (
      typeof this.#remoteAddress === "undefined" ||
      typeof this.#remotePort === "undefined"
    ) {
      return codeMap.get("EADDRNOTAVAIL")!;
    }

    peername.address = this.#remoteAddress;
    peername.port = this.#remotePort;
    peername.family = this.#remoteFamily;

    return 0;
  }

  /**
   * @param noDelay
   * @return An error status code.
   */
  setNoDelay(noDelay: boolean): number {
    if (this.#useNative) {
      return this.#native.setNoDelay(noDelay);
    }

    if (this[kStreamBaseField] && "setNoDelay" in this[kStreamBaseField]) {
      this[kStreamBaseField].setNoDelay(noDelay);
    }
    return 0;
  }

  /**
   * @param enable
   * @param initialDelay
   * @return An error status code.
   */
  setKeepAlive(_enable: boolean, _initialDelay: number): number {
    // TODO(bnoordhuis) https://github.com/denoland/deno/pull/13103
    return 0;
  }

  /**
   * Windows only.
   *
   * Deprecated by Node.
   * REF: https://github.com/nodejs/node/blob/master/lib/net.js#L1731
   *
   * @param enable
   * @return An error status code.
   * @deprecated
   */
  setSimultaneousAccepts(_enable: boolean) {
    // Low priority to implement owing to it being deprecated in Node.
    notImplemented("TCP.prototype.setSimultaneousAccepts");
  }

  /**
   * Legacy listen using Deno.listen.
   */
  #listenLegacy(backlog: number): number {
    this.#backlog = ceilPowOf2(backlog + 1);

    const listenOptions = {
      hostname: this.#address!,
      port: this.#port!,
      transport: "tcp" as const,
    };

    let listener;

    try {
      listener = Deno.listen(listenOptions);
    } catch (e) {
      if (e instanceof Deno.errors.NotCapable) {
        throw e;
      }
      return codeMap.get(e.code ?? "UNKNOWN") ?? codeMap.get("UNKNOWN")!;
    }

    const address = listener.addr as Deno.NetAddr;
    this.#address = address.hostname;
    this.#port = address.port;
    this.#listener = listener;

    nextTick(nextTick, () => this.#accept());

    return 0;
  }

  /**
   * Connect to an IPv4 or IPv6 address.
   * @param req A TCPConnectWrap instance.
   * @param address The hostname to connect to.
   * @param port The port to connect to.
   * @return An error status code.
   */
  #connect(req: TCPConnectWrap, address: string, port: number): number {
    this.#remoteAddress = address;
    this.#remotePort = port;
    this.#remoteFamily = getIPFamily(address);

    op_net_connect_tcp(
      { hostname: address ?? "127.0.0.1", port },
      this.#netPermToken,
    ).then(
      ({ 0: rid, 1: localAddr, 2: remoteAddr }) => {
        // Incorrect / backwards, but correcting the local address and port with
        // what was actually used given we can't actually specify these in Deno.
        this.#address = req.localAddress = localAddr.hostname;
        this.#port = req.localPort = localAddr.port;
        this[kStreamBaseField] = new TcpConn(rid, remoteAddr, localAddr);

        try {
          this.afterConnect(req, 0);
        } catch {
          // swallow callback errors.
        }
      },
      () => {
        try {
          // TODO(cmorten): correct mapping of connection error to status code.
          this.afterConnect(req, codeMap.get("ECONNREFUSED")!);
        } catch {
          // swallow callback errors.
        }
      },
    );

    return 0;
  }

  /** Handle backoff delays following an unsuccessful accept. */
  async #acceptBackoff() {
    // Backoff after transient errors to allow time for the system to
    // recover, and avoid blocking up the event loop with a continuously
    // running loop.
    if (!this.#acceptBackoffDelay) {
      this.#acceptBackoffDelay = INITIAL_ACCEPT_BACKOFF_DELAY;
    } else {
      this.#acceptBackoffDelay *= 2;
    }

    if (this.#acceptBackoffDelay >= MAX_ACCEPT_BACKOFF_DELAY) {
      this.#acceptBackoffDelay = MAX_ACCEPT_BACKOFF_DELAY;
    }

    await delay(this.#acceptBackoffDelay);

    this.#accept();
  }

  /** Accept new connections (legacy path). */
  async #accept(): Promise<void> {
    if (this.#closed) {
      return;
    }

    if (this.#connections > this.#backlog!) {
      this.#acceptBackoff();

      return;
    }

    let connection: Deno.Conn;

    try {
      connection = await this.#listener.accept();
    } catch (e) {
      if (e instanceof Deno.errors.BadResource && this.#closed) {
        // Listener and server has closed.
        return;
      }

      try {
        // TODO(cmorten): map errors to appropriate error codes.
        this.onconnection!(codeMap.get("UNKNOWN")!, undefined);
      } catch {
        // swallow callback errors.
      }

      this.#acceptBackoff();

      return;
    }

    // Reset the backoff delay upon successful accept.
    this.#acceptBackoffDelay = undefined;
    const connectionHandle = new TCP(socketType.SOCKET, connection);
    this.#connections++;

    try {
      this.onconnection!(0, connectionHandle);
    } catch {
      // swallow callback errors.
    }

    return this.#accept();
  }

  /** Handle server closure. */
  override _onClose(): number {
    this.#closed = true;
    this.reading = false;

    this.#address = undefined;
    this.#port = undefined;

    this.#remoteAddress = undefined;
    this.#remoteFamily = undefined;
    this.#remotePort = undefined;

    this.#backlog = undefined;
    this.#connections = 0;
    this.#acceptBackoffDelay = undefined;

    // Close native libuv handle after pending writes flush.
    // uv_run processes pending writes each event loop tick,
    // setTimeout(0) ensures close runs after that.
    if (this.#native) {
      const native = this.#native;
      this.#native = null;
      setTimeout(() => native.close(), 0);
    }

    if (
      !this.#useNative &&
      this.provider === providerType.TCPSERVERWRAP
    ) {
      try {
        this.#listener.close();
      } catch {
        // listener already closed
      }
    }

    return LibuvStreamWrap.prototype._onClose.call(this);
  }

  setNetPermToken(netPermToken: object | undefined) {
    this.#netPermToken = netPermToken;
  }
}
