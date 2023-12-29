// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import { Conn, Listener } from "ext:deno_net/01_net.js";
const { Number, TypeError } = primordials;
const {
  op_tls_handshake,
  op_tls_start,
  op_net_accept_tls,
  op_net_connect_tls,
} = core.ensureFastOps();

function opStartTls(args) {
  return op_tls_start(args);
}

function opTlsHandshake(rid) {
  return op_tls_handshake(rid);
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
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tls(
    { hostname, port },
    { certFile, caCerts, certChain, privateKey, alpnProtocols },
  );
  localAddr.transport = "tcp";
  remoteAddr.transport = "tcp";
  return new TlsConn(rid, remoteAddr, localAddr);
}

class TlsListener extends Listener {
  async accept() {
    const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_accept_tls(
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
    { hostname, port: Number(port) },
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
