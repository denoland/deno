// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { Listener, Conn } = window.__bootstrap.net;

  function opConnectTls(
    args,
  ) {
    return core.jsonOpAsync("op_connect_tls", args);
  }

  function opAcceptTLS(rid) {
    return core.jsonOpAsync("op_accept_tls", { rid });
  }

  function opListenTls(args) {
    return core.jsonOpSync("op_listen_tls", args);
  }

  function opStartTls(args) {
    return core.jsonOpAsync("op_start_tls", args);
  }

  async function connectTls({
    port,
    hostname = "127.0.0.1",
    transport = "tcp",
    certFile = undefined,
    certChain = undefined,
    privateKey = undefined
  }) {
    const res = await opConnectTls({
      port,
      hostname,
      transport,
      certFile,
      certChain,
      privateKey
    });
    return new Conn(res.rid, res.remoteAddr, res.localAddr);
  }

  class TLSListener extends Listener {
    async accept() {
      const res = await opAcceptTLS(this.rid);
      return new Conn(res.rid, res.remoteAddr, res.localAddr);
    }
  }

  function listenTls({
    port,
    certFile,
    keyFile,
    hostname = "0.0.0.0",
    transport = "tcp",
  }) {
    const res = opListenTls({
      port,
      certFile,
      keyFile,
      hostname,
      transport,
    });
    return new TLSListener(res.rid, res.localAddr);
  }

  async function startTls(
    conn,
    { hostname = "127.0.0.1", certFile } = {},
  ) {
    const res = await opStartTls({
      rid: conn.rid,
      hostname,
      certFile,
    });
    return new Conn(res.rid, res.remoteAddr, res.localAddr);
  }

  window.__bootstrap.tls = {
    startTls,
    listenTls,
    connectTls,
    TLSListener,
  };
})(this);
