// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const net = window.__bootstrap.net;

  function listen(options) {
    if (options.transport === "unix") {
      const res = net.opListen(options);
      return new net.Listener(res.rid, res.localAddr);
    } else {
      return net.listen(options);
    }
  }

  function listenDatagram(
    options,
  ) {
    let res;
    if (options.transport === "unixpacket") {
      res = net.opListen(options);
    } else {
      res = net.opListen({
        transport: "udp",
        hostname: "127.0.0.1",
        ...options,
      });
    }

    return new net.Datagram(res.rid, res.localAddr);
  }

  async function connect(
    options,
  ) {
    if (options.transport === "unix") {
      const res = await net.opConnect(options);
      return new net.Conn(res.rid, res.remoteAddr, res.localAddr);
    } else {
      return net.connect(options);
    }
  }

  window.__bootstrap.netUnstable = {
    connect,
    listenDatagram,
    listen,
  };
})(this);
