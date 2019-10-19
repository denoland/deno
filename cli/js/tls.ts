// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendAsync, sendSync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { Addr, Listener, Transport, Conn, ConnImpl } from "./net.ts";
import { close } from "./files.ts";

// TODO(ry) There are many configuration options to add...
// https://docs.rs/rustls/0.16.0/rustls/struct.ClientConfig.html
interface DialTLSOptions {
  port: number;
  hostname?: string;
  certFile?: string;
}
const dialTLSDefaults = { hostname: "127.0.0.1", transport: "tcp" };

/**
 * dialTLS establishes a secure connection over TLS (transport layer security).
 */
export async function dialTLS(options: DialTLSOptions): Promise<Conn> {
  options = Object.assign(dialTLSDefaults, options);
  const res = await sendAsync(dispatch.OP_DIAL_TLS, options);
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}

class TLSListenerImpl implements Listener {
  constructor(
    readonly rid: number,
    private transport: Transport,
    private localAddr: string
  ) {}

  async accept(): Promise<Conn> {
    const res = await sendAsync(dispatch.OP_ACCEPT_TLS, { rid: this.rid });
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }

  close(): void {
    close(this.rid);
  }

  addr(): Addr {
    return {
      transport: this.transport,
      address: this.localAddr
    };
  }

  async next(): Promise<IteratorResult<Conn>> {
    return {
      done: false,
      value: await this.accept()
    };
  }

  [Symbol.asyncIterator](): AsyncIterator<Conn> {
    return this;
  }
}

export interface ListenTLSOptions {
  port: number;
  hostname?: string;
  transport?: Transport;
  certFile: string;
  keyFile: string;
}

export function listenTLS(options: ListenTLSOptions): Listener {
  const hostname = options.hostname || "0.0.0.0";
  const transport = options.transport || "tcp";
  const res = sendSync(dispatch.OP_LISTEN_TLS, {
    hostname,
    port: options.port,
    transport,
    certFile: options.certFile,
    keyFile: options.keyFile
  });
  return new TLSListenerImpl(res.rid, transport, res.localAddr);
}
