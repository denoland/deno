// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const net = require("net");

process.on("uncaughtException", function (error) {
  console.error(error);
});

if (process.argv.length != 4) {
  console.log("usage: %s <localport> <remoteport>", process.argv[1]);
  process.exit();
}

const localport = process.argv[2];
const remoteport = process.argv[3];

const remotehost = "127.0.0.1";

const server = net.createServer(function (localsocket) {
  const remotesocket = new net.Socket();

  remotesocket.connect(remoteport, remotehost);

  localsocket.on("data", function (data) {
    const flushed = remotesocket.write(data);
    if (!flushed) {
      localsocket.pause();
    }
  });

  remotesocket.on("data", function (data) {
    const flushed = localsocket.write(data);
    if (!flushed) {
      remotesocket.pause();
    }
  });

  localsocket.on("drain", function () {
    remotesocket.resume();
  });

  remotesocket.on("drain", function () {
    localsocket.resume();
  });

  localsocket.on("close", function () {
    remotesocket.end();
  });

  remotesocket.on("close", function () {
    localsocket.end();
  });

  localsocket.on("error", function () {
    localsocket.end();
  });

  remotesocket.on("error", function () {
    remotesocket.end();
  });
});

server.listen(localport);

console.log(
  "redirecting connections from 127.0.0.1:%d to %s:%d",
  localport,
  remotehost,
  remoteport,
);
