// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { core, ops } from "internal:core/01_core.js";
import { Conn, Listener } from "internal:ext/net/01_net.js";
import primordials from "internal:core/00_primordials.js";
const { TypeError } = primordials;

function opStartTls(args) {
  return core.opAsync("op_tls_start", args);
}

function opTlsHandshake(rid) {
  return core.opAsync("op_tls_handshake", rid);
}

class TlsConn extends Conn {
  handshake() {
    return opTlsHandshake(this.rid);
  }
}

async function connectTls({
  port,
  hostname = "127.0.0.1",
  transport = "tcp",
  certFile = undefined,
  caCerts = [],
  certChain = undefined,
  privateKey = undefined,
  alpnProtocols = undefined,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await core.opAsync(
    "op_net_connect_tls",
    { hostname, port },
    { certFile, caCerts, certChain, privateKey, alpnProtocols },
  );
  localAddr.transport = "tcp";
  remoteAddr.transport = "tcp";
  return new TlsConn(rid, remoteAddr, localAddr);
}

class TlsListener extends Listener {
  async accept() {
    const { 0: rid, 1: localAddr, 2: remoteAddr } = await core.opAsync(
      "op_net_accept_tls",
      this.rid,
    );
    localAddr.transport = "tcp";
    remoteAddr.transport = "tcp";
    return new TlsConn(rid, remoteAddr, localAddr);
  }
}

function listenTls({
  port,
  cert,
  certFile,
  key,
  keyFile,
  hostname = "0.0.0.0",
  transport = "tcp",
  alpnProtocols = undefined,
  reusePort = false,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  const { 0: rid, 1: localAddr } = ops.op_net_listen_tls(
    { hostname, port },
    { cert, certFile, key, keyFile, alpnProtocols, reusePort },
  );
  return new TlsListener(rid, localAddr);
}

async function startTls(
  conn,
  {
    hostname = "127.0.0.1",
    certFile = undefined,
    caCerts = [],
    alpnProtocols = undefined,
  } = {},
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await opStartTls({
    rid: conn.rid,
    hostname,
    certFile,
    caCerts,
    alpnProtocols,
  });
  return new TlsConn(rid, remoteAddr, localAddr);
}

export { connectTls, listenTls, startTls, TlsConn, TlsListener };
