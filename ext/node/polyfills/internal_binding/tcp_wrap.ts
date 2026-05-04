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

// TCPWrap is a Rust CppGC object that inherits from LibUvStreamWrap.
// Read/write/shutdown ops come from the LibUvStreamWrap base class.
// TCP-specific ops (bind, listen, connect, accept, etc.) are on TCPWrap itself.
//
// This module adds thin JS wrappers for listen (to create client handles on
// accept) and for re-exporting types.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials
// deno-fmt-ignore-file
(function () {
  const { core } = globalThis.__bootstrap;
  const { TCPWrap } = core.ops;
  const { AsyncWrap, providerType } = core.loadExtScript("ext:deno_node/internal_binding/async_wrap.ts");

  // Mark TCPWrap as a StreamBase handle, matching Node's StreamBase::AddMethods.
  // This allows parser.consume(socket._handle) to detect it as consumable.
  TCPWrap.prototype.isStreamBase = true;

  /** The type of TCP socket. */
  enum socketType {
    SOCKET,
    SERVER,
  }

  class TCPConnectWrap extends AsyncWrap {
    oncomplete!: (
      status: number,
      handle: unknown,
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

  enum constants {
    SOCKET = socketType.SOCKET,
    SERVER = socketType.SERVER,
    UV_TCP_IPV6ONLY,
    UV_TCP_REUSEPORT = 4,
  }

  /**
   * Wrap the native TCPWrap.listen() to handle connection acceptance.
   * The Rust server_connection_cb fires onconnection(status), and this
   * wrapper creates client handles and calls uv_accept before forwarding
   * to the user's onconnection(status, clientHandle).
   *
   * TODO: Move this logic into Rust by making the connection callback
   * allocate a CppGC TCPWrap directly, removing the need for this JS shim.
   */
  function setupListenWrap(serverHandle: InstanceType<typeof TCPWrap>) {
    const userOnConnection = serverHandle.onconnection;
    serverHandle.onconnection = function (status: number) {
      if (status !== 0) {
        if (userOnConnection) {
          userOnConnection.call(serverHandle, status, undefined);
        }
        return;
      }

      // Create a new client handle and accept the connection
      const clientHandle = new TCPWrap(socketType.SOCKET);
      const acceptErr = serverHandle.accept(clientHandle);
      if (acceptErr !== 0) {
        if (userOnConnection) {
          userOnConnection.call(serverHandle, acceptErr, undefined);
        }
        return;
      }

      if (userOnConnection) {
        userOnConnection.call(serverHandle, 0, clientHandle);
      }
    };
  }

  // Re-export the Rust TCPWrap as TCP.

  const _defaultExport = {
    TCPConnectWrap,
    constants,
    TCP: TCPWrap,
    setupListenWrap,
  };

  return {
    TCP: TCPWrap,
    setupListenWrap,
    TCPConnectWrap,
    socketType,
    constants,
    default: _defaultExport,
  };
})()
