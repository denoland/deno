// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const { Listener, Conn } = window.__bootstrap.net;

  function opConnectTls(
    args,
  ) {
    return core.opAsync("op_tls_connect", args);
  }

  function opAcceptTLS(rid) {
    return core.opAsync("op_tls_accept", rid);
  }

  function opListenTls(args) {
    return ops.op_tls_listen(args);
  }

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
    const res = await opConnectTls({
      port,
      hostname,
      transport,
      certFile,
      caCerts,
      certChain,
      privateKey,
      alpnProtocols,
    });
    return new TlsConn(res.rid, res.remoteAddr, res.localAddr);
  }

  class TlsListener extends Listener {
    async accept() {
      const res = await opAcceptTLS(this.rid);
      return new TlsConn(res.rid, res.remoteAddr, res.localAddr);
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
  }) {
    const res = opListenTls({
      port,
      cert,
      certFile,
      key,
      keyFile,
      hostname,
      transport,
      alpnProtocols,
    });
    return new TlsListener(res.rid, res.localAddr);
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
    const res = await opStartTls({
      rid: conn.rid,
      hostname,
      certFile,
      caCerts,
      alpnProtocols,
    });
    return new TlsConn(res.rid, res.remoteAddr, res.localAddr);
  }

  window.__bootstrap.tls = {
    startTls,
    listenTls,
    connectTls,
    TlsConn,
    TlsListener,
  };
})(this);
