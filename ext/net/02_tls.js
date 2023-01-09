// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const { Listener, Conn } = window.__bootstrap.net;
  const { TypeError } = window.__bootstrap.primordials;

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
    const [rid, localAddr, remoteAddr] = await core.opAsync(
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
      const [rid, localAddr, remoteAddr] = await core.opAsync(
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
    const [rid, localAddr] = ops.op_net_listen_tls(
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
    const [rid, localAddr, remoteAddr] = await opStartTls({
      rid: conn.rid,
      hostname,
      certFile,
      caCerts,
      alpnProtocols,
    });
    return new TlsConn(rid, remoteAddr, localAddr);
  }

  window.__bootstrap.tls = {
    startTls,
    listenTls,
    connectTls,
    TlsConn,
    TlsListener,
  };
})(this);
