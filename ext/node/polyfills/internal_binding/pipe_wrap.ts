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
// - https://github.com/nodejs/node/blob/master/src/pipe_wrap.cc
// - https://github.com/nodejs/node/blob/master/src/pipe_wrap.h
//
// PipeWrap is a Rust CppGC object that inherits from LibUvStreamWrap.
// Read/write/shutdown ops come from the LibUvStreamWrap base class.
// Pipe-specific ops (bind, listen, connect, accept, open, fchmod,
// setPendingInstances) are on PipeWrap itself.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials
// deno-fmt-ignore-file
(function () {
  const { core, primordials } = globalThis.__bootstrap;
  const { op_node_create_pipe, PipeWrap } = core.ops;
  const { AsyncWrap, providerType } = core.loadExtScript("ext:deno_node/internal_binding/async_wrap.ts");
  const { ceilPowOf2 } = core.loadExtScript("ext:deno_node/internal_binding/_listen.ts");
  const { codeMap } = core.loadExtScript("ext:deno_node/internal_binding/uv.ts");
  const { fs } = core.loadExtScript(
    "ext:deno_node/internal_binding/constants.ts",
  );

  const { MapPrototypeGet } = primordials;

  // Mark PipeWrap as a StreamBase handle, matching Node's StreamBase::AddMethods.
  PipeWrap.prototype.isStreamBase = true;

  /** The type of pipe socket. */
  enum socketType {
    SOCKET,
    SERVER,
    IPC,
  }

  enum constants {
    SOCKET = socketType.SOCKET,
    SERVER = socketType.SERVER,
    IPC = socketType.IPC,
    UV_READABLE = 1,
    UV_WRITABLE = 2,
  }

  class PipeConnectWrap extends AsyncWrap {
    oncomplete!: (
      status: number,
      handle: unknown,
      req: PipeConnectWrap,
      readable: boolean,
      writeable: boolean,
    ) => void;
    address!: string;

    constructor() {
      super(providerType.PIPECONNECTWRAP);
    }
  }

  // Translate UV_READABLE/UV_WRITABLE flags to POSIX mode bits before calling
  // the native fchmod op (which takes raw chmod bits).
  const nativeFchmod = PipeWrap.prototype.fchmod;
  PipeWrap.prototype.fchmod = function (mode: number): number {
    if (
      mode !== constants.UV_READABLE &&
      mode !== constants.UV_WRITABLE &&
      mode !== (constants.UV_WRITABLE | constants.UV_READABLE)
    ) {
      return MapPrototypeGet(codeMap, "EINVAL");
    }

    let desiredMode = 0;
    if (mode & constants.UV_READABLE) {
      desiredMode |= fs.S_IRUSR | fs.S_IRGRP | fs.S_IROTH;
    }
    if (mode & constants.UV_WRITABLE) {
      desiredMode |= fs.S_IWUSR | fs.S_IWGRP | fs.S_IWOTH;
    }

    return nativeFchmod.call(this, desiredMode);
  };

  // Round up the backlog to the next power of two (matching the previous
  // implementation). TCP uses the raw backlog; pipes historically rounded.
  const nativeListen = PipeWrap.prototype.listen;
  PipeWrap.prototype.listen = function (backlog: number): number {
    return nativeListen.call(this, ceilPowOf2(backlog + 1));
  };

  /**
   * Wrap the native PipeWrap.listen() to handle connection acceptance.
   * The Rust server_connection_cb fires onconnection(status), and this
   * wrapper creates client handles and calls uv_accept before forwarding
   * to the user's onconnection(status, clientHandle).
   */
  function setupListenWrap(serverHandle: InstanceType<typeof PipeWrap>) {
    const userOnConnection = serverHandle.onconnection;
    serverHandle.onconnection = function (status: number) {
      if (status !== 0) {
        if (userOnConnection) {
          userOnConnection.call(serverHandle, status, undefined);
        }
        return;
      }

      const clientHandle = new PipeWrap(socketType.SOCKET);
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

  // Re-export the Rust PipeWrap as Pipe.

  /** Create an anonymous pipe pair. Returns [readFd, writeFd]. */
  function createPipe(): [number, number] {
    return op_node_create_pipe();
  }

  const _defaultExport = {
    Pipe: PipeWrap,
    PipeConnectWrap,
    constants,
    setupListenWrap,
    createPipe,
  };

  return {
    Pipe: PipeWrap,
    setupListenWrap,
    createPipe,
    PipeConnectWrap,
    socketType,
    constants,
    default: _defaultExport,
  };
})()
