// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as tlsOps from "./ops/tls.ts";
import { Listener, Conn, ConnImpl, ListenerImpl } from "./net.ts";

// TODO(ry) There are many configuration options to add...
// https://docs.rs/rustls/0.16.0/rustls/struct.ClientConfig.html
interface ConnectTlsOptions {
  transport?: "tcp";
  port: number;
  hostname?: string;
  certFile?: string;
}

export async function connectTls({
  port,
  hostname = "127.0.0.1",
  transport = "tcp",
  certFile = undefined,
}: ConnectTlsOptions): Promise<Conn> {
  const res = await tlsOps.connectTls({
    port,
    hostname,
    transport,
    certFile,
  });
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}

class TLSListenerImpl extends ListenerImpl {
  async accept(): Promise<Conn> {
    const res = await tlsOps.acceptTLS(this.rid);
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }
}

export interface ListenTlsOptions {
  port: number;
  hostname?: string;
  transport?: "tcp";
  certFile: string;
  keyFile: string;
}

export function listenTls({
  port,
  certFile,
  keyFile,
  hostname = "0.0.0.0",
  transport = "tcp",
}: ListenTlsOptions): Listener {
  const res = tlsOps.listenTls({
    port,
    certFile,
    keyFile,
    hostname,
    transport,
  });
  return new TLSListenerImpl(res.rid, res.localAddr);
}

interface StartTlsOptions {
  hostname?: string;
  certFile?: string;
}

export async function startTls(
  conn: Conn,
  { hostname = "127.0.0.1", certFile = undefined }: StartTlsOptions = {}
): Promise<Conn> {
  const res = await tlsOps.startTls({
    rid: conn.rid,
    hostname,
    certFile,
  });
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
