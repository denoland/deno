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

(function () {
const { core, primordials } = __bootstrap;
const {
  op_node_udp_recv,
  op_node_udp_send,
  SendWrap,
  UDP: NativeUDP,
} = core.ops;
const {
  ObjectPrototypeIsPrototypeOf,
  Uint8Array,
} = primordials;

core.loadExtScript("ext:deno_node/internal_binding/handle_wrap.ts");
const { ownerSymbol } = core.loadExtScript(
  "ext:deno_node/internal_binding/symbols.ts",
);
const { codeMap } = core.loadExtScript("ext:deno_node/internal_binding/uv.ts");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { isIP } = core.loadExtScript("ext:deno_node/internal/net.ts");

type MessageType = string | Uint8Array | Buffer | DataView;
type SendWrapInstance = InstanceType<typeof SendWrap>;

const AF_INET = 2;
const AF_INET6 = 10;

class UDP extends NativeUDP {
  [ownerSymbol]: unknown = null;

  #receiving = false;
  #unrefed = false;

  onmessage!: (
    nread: number,
    handle: UDP,
    buf?: Buffer,
    rinfo?: {
      address: string;
      family: "IPv4" | "IPv6";
      port: number;
      size?: number;
    },
  ) => void;

  lookup!: (
    address: string,
    callback: (
      err: Error | null,
      address: string,
      family: number,
    ) => void,
    // deno-lint-ignore no-explicit-any
  ) => any;

  constructor() {
    super();
  }

  /**
   * Bind to an IPv4 address.
   * @param ip The hostname to bind to.
   * @param port The port to bind to
   * @return An error status code.
   */
  declare bind: (ip: string, port: number, flags: number) => number;

  /**
   * Bind to an IPv6 address.
   * @param ip The hostname to bind to.
   * @param port The port to bind to
   * @return An error status code.
   */
  declare bind6: (ip: string, port: number, flags: number) => number;

  declare bufferSize: (
    size: number,
    buffer: boolean,
    ctx: Record<string, string | number>,
  ) => number | undefined;

  declare connect: (ip: string, port: number) => number;
  declare connect6: (ip: string, port: number) => number;
  declare disconnect: () => number;
  declare addMembership: (
    multicastAddress: string,
    interfaceAddress?: string,
  ) => number;
  declare dropMembership: (
    multicastAddress: string,
    interfaceAddress?: string,
  ) => number;
  declare addSourceSpecificMembership: (
    sourceAddress: string,
    groupAddress: string,
    interfaceAddress?: string,
  ) => number;
  declare dropSourceSpecificMembership: (
    sourceAddress: string,
    groupAddress: string,
    interfaceAddress?: string,
  ) => number;

  /**
   * Populates the provided object with remote address entries.
   * @param peername An object to add the remote address entries to.
   * @return An error status code.
   */
  declare getpeername: (peername: Record<string, string | number>) => number;

  /**
   * Populates the provided object with local address entries.
   * @param sockname An object to add the local address entries to.
   * @return An error status code.
   */
  declare getsockname: (sockname: Record<string, string | number>) => number;

  /**
   * Opens an existing file descriptor as this UDP socket.
   * @param fd The file descriptor to open.
   * @return An error status code.
   */
  declare open: (fd: number) => number;

  /**
   * Return the raw fd so it can be sent over IPC via SCM_RIGHTS.
   * Returns -1 on platforms that don't support fd-passing.
   */
  declare fdForIpc: () => number;

  /**
   * Start receiving on the connection.
   * @return An error status code.
   */
  recvStart(): number {
    if (!this.#receiving) {
      this.#receiving = true;
      this.#receive();
    }

    return 0;
  }

  /**
   * Stop receiving on the connection.
   * @return An error status code.
   */
  recvStop(): number {
    this.#receiving = false;

    return 0;
  }

  override ref() {
    this.#unrefed = false;
  }

  send(
    req: SendWrapInstance,
    bufs: MessageType[],
    count: number,
    ...args: [number, string, boolean] | [boolean]
  ): number {
    return this.#doSend(req, bufs, count, args, AF_INET);
  }

  send6(
    req: SendWrapInstance,
    bufs: MessageType[],
    count: number,
    ...args: [number, string, boolean] | [boolean]
  ): number {
    return this.#doSend(req, bufs, count, args, AF_INET6);
  }

  declare setBroadcast: (bool: 0 | 1) => number;
  declare setMulticastInterface: (interfaceAddress: string) => number;
  declare setMulticastLoopback: (bool: 0 | 1) => number;
  declare setMulticastTTL: (ttl: number) => number;
  declare setTTL: (ttl: number) => number;
  declare _rid: () => number;
  declare _setRemote: (ip: string, port: number, family: number) => number;
  declare _remoteAddress: () => string | undefined;
  declare _remotePort: () => number;
  declare _recvBufferSize: () => number;
  declare _closeResource: () => void;

  override unref() {
    this.#unrefed = true;
  }

  #doSend(
    req: SendWrapInstance,
    bufs: MessageType[],
    count: number,
    args: [number, string, boolean] | [boolean],
    _family: number,
  ): number {
    let hasCallback: boolean;

    if (args.length === 3) {
      this._setRemote(args[1] as string, args[0] as number, _family);
      hasCallback = args[2] as boolean;
    } else {
      hasCallback = args[0] as boolean;
    }

    const promise = op_node_udp_send(
      this._rid(),
      bufs,
      count,
      this._remoteAddress()!,
      this._remotePort(),
    );
    if (hasCallback) {
      promise.then(({ err, sent }) => {
        try {
          req.oncomplete(err, sent);
        } catch {
          // swallow callback errors
        }
      });
    }

    return 0;
  }

  async #receive() {
    if (!this.#receiving) {
      return;
    }

    const p = new Uint8Array(this._recvBufferSize());

    let nread: number;
    let remoteHostname: string | null = null;
    let remotePort: number | null = null;

    try {
      const promise = op_node_udp_recv(this._rid(), p);
      if (this.#unrefed) {
        core.unrefOpPromise(promise);
      }
      const result = await promise;
      nread = result.nread;
      remoteHostname = result.hostname;
      remotePort = result.port;
    } catch (e) {
      if (
        ObjectPrototypeIsPrototypeOf(Deno.errors.Interrupted.prototype, e) ||
        ObjectPrototypeIsPrototypeOf(Deno.errors.BadResource.prototype, e)
      ) {
        nread = 0;
      } else {
        nread = codeMap.get("UNKNOWN")!;
      }
    }

    const rinfo = remoteHostname !== null
      ? {
        address: remoteHostname,
        port: remotePort!,
        family: isIP(remoteHostname) === 6
          ? ("IPv6" as const)
          : ("IPv4" as const),
      }
      : undefined;

    const buf = remoteHostname !== null
      // deno-lint-ignore prefer-primordials
      ? Buffer.from(p.buffer, p.byteOffset, nread)
      : Buffer.alloc(0);

    try {
      this.onmessage(nread, this, buf, rinfo);
    } catch {
      // swallow callback errors.
    }

    this.#receive();
  }

  /** Handle socket closure. */
  override _onClose(): number {
    this.#receiving = false;

    this._closeResource();

    return 0;
  }
}

return {
  default: { SendWrap, UDP },
  SendWrap,
  UDP,
};
})();
