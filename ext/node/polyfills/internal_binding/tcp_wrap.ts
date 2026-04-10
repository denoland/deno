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

import { TCP as NativeTCP } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
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
  kLastWriteWasAsync,
  kReadBytesOrError,
  LibuvStreamWrap,
  ShutdownWrap,
  streamBaseState,
  WriteWrap,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
import { ownerSymbol } from "ext:deno_node/internal_binding/symbols.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { getIPFamily } from "ext:deno_node/internal/net.ts";
import { ceilPowOf2 } from "ext:deno_node/internal_binding/_listen.ts";
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
  #connections = 0;

  #closed = false;

  // deno-lint-ignore no-explicit-any -- Native libuv TCP handle
  #native: any;

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
    return undefined;
  }

  get _nativeHandle() {
    return this.#native;
  }

  /**
   * Opens a file descriptor.
   * @param fd The file descriptor to open.
   * @return An error status code.
   */
  open(fd: number): number {
    return this.#native.open(fd);
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
    this.#backlog = ceilPowOf2(backlog + 1);

    // deno-lint-ignore no-this-alias
    const self = this;
    this.#native.onconnection = function (status: number) {
      if (status !== 0) {
        try {
          self.onconnection!(status, undefined);
        } catch (_e) {
          // swallow callback errors.
        }
        return;
      }

      self.#connections++;
      const clientHandle = new TCP(socketType.SOCKET);
      self.#native.accept(clientHandle.#native);
      clientHandle.#native.setOwner();

      try {
        self.onconnection!(0, clientHandle);
      } catch (_e) {
        // swallow callback errors.
      }
    };

    return this.#native.listen(this.#backlog);
  }

  override readStart(): number {
    this.reading = true;
    // deno-lint-ignore no-this-alias
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
        self.onread!(buf ?? new Uint8Array(0), nread);
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
    this.reading = false;
    return this.#native.readStop();
  }

  override writeBuffer(
    req: WriteWrap<LibuvStreamWrap>,
    data: Uint8Array,
  ): number {
    this.#native.writeBuffer(data);
    streamBaseState[kBytesWritten] = data.byteLength;
    // The native writeBuffer is synchronous (data is queued in libuv),
    // so mark as sync. afterWriteDispatched will call the callback
    // immediately, which is needed for the Writable's clearBuffer to
    // process corked writes correctly.
    streamBaseState[kLastWriteWasAsync] = 0;
    this.bytesWritten += data.byteLength;

    return 0;
  }

  override writev(
    req: WriteWrap<LibuvStreamWrap>,
    chunks: Buffer[] | (string | Buffer)[],
    allBuffers: boolean,
  ): number {
    // Concat all chunks and write as single buffer
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

    return this.writeBuffer(req, Buffer.concat(buffers));
  }

  override shutdown(req: ShutdownWrap<LibuvStreamWrap>): number {
    // Call uv_shutdown to send FIN to the remote side.
    const ret = this.#native!.shutdown();
    // Signal async completion like Node.js - the FIN is queued
    // in libuv and will be sent asynchronously.
    nextTick(() => {
      try {
        req.oncomplete(ret);
      } catch {
        // swallow callback errors.
      }
    });

    return ret;
  }

  override ref() {
    if (this.#native) {
      this.#native.ref();
    }
  }

  override unref() {
    if (this.#native) {
      this.#native.unref();
    }
  }

  /**
   * Populates the provided object with local address entries.
   * @param sockname An object to add the local address entries to.
   * @return An error status code.
   */
  getsockname(sockname: Record<string, never> | AddressInfo): number {
    if (this.#native) {
      const info = this.#native.getsockname();
      if (info) {
        sockname.address = info.address;
        sockname.port = info.port;
        sockname.family = info.family;
        return 0;
      }
    }
    return codeMap.get("EADDRNOTAVAIL")!;
  }

  /**
   * Populates the provided object with remote address entries.
   * @param peername An object to add the remote address entries to.
   * @return An error status code.
   */
  getpeername(peername: Record<string, never> | AddressInfo): number {
    if (this.#native) {
      const info = this.#native.getpeername();
      if (info) {
        peername.address = info.address;
        peername.port = info.port;
        peername.family = info.family;
        return 0;
      }
    }
    return codeMap.get("EADDRNOTAVAIL")!;
  }

  /**
   * @param noDelay
   * @return An error status code.
   */
  setNoDelay(noDelay: boolean): number {
    return this.#native.setNoDelay(noDelay);
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

  #connect(req: TCPConnectWrap, address: string, port: number): number {
    this.#remoteAddress = address;
    this.#remotePort = port;
    this.#remoteFamily = getIPFamily(address);

    // deno-lint-ignore no-this-alias
    const self = this;
    this.#native.onconnect = function (status: number) {
      if (status === 0) {
        // Populate local address from the native handle
        const sockname = self.#native.getsockname();
        if (sockname) {
          self.#address = req.localAddress = sockname.address;
          self.#port = req.localPort = sockname.port;
        }

        // Populate remote address from the native handle
        const peername = self.#native.getpeername();
        if (peername) {
          self.#remoteAddress = peername.address;
          self.#remotePort = peername.port;
          self.#remoteFamily = peername.family;
        }

        try {
          self.afterConnect(req, 0);
        } catch {
          // swallow callback errors.
        }
      } else {
        try {
          self.afterConnect(req, codeMap.get("ECONNREFUSED")!);
        } catch {
          // swallow callback errors.
        }
      }
    };

    const ret = this.#native.connect(address ?? "127.0.0.1", port);
    if (ret !== 0) {
      // Synchronous failure (e.g. bad address)
      nextTick(() => {
        try {
          this.afterConnect(req, codeMap.get("ECONNREFUSED")!);
        } catch {
          // swallow callback errors.
        }
      });
    }

    return 0;
  }

  setNetPermToken(_netPermToken: object | undefined) {
    // No-op: permission tokens were used by the old op_net_connect_tcp path.
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

    // Close native libuv handle. uv_close is safe to call at any time -
    // it cancels pending writes and defers the actual handle free to the
    // close callback (which fires during the next uv_run).
    if (this.#native) {
      this.#native.close();
      this.#native = null;
    }

    return LibuvStreamWrap.prototype._onClose.call(this);
  }
}
