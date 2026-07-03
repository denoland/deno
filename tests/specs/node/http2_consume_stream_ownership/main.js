// Regression test: handing a TCP handle to an Http2Session via
// `consumeStream` must transfer ownership of the underlying libuv handle to
// the session. If the TCPWrap kept ownership too, destroying the session and
// then closing the TCP handle would free the same allocation twice (a double
// free / use-after-free) and crash the process on shutdown.
import http2 from "node:http2";
import { Duplex } from "node:stream";
import process from "node:process";

const { TCP } = process.binding("tcp_wrap");
const tcp = new TCP(0);

// A dummy duplex stream lets us create a session without an actual network
// connection (so no permissions are required).
const dummySocket = new Duplex({
  read() {},
  write(_chunk, _encoding, callback) {
    callback();
  },
});

const session = http2.connect("http://localhost", {
  createConnection: () => dummySocket,
});

const kHandle = Object.getOwnPropertySymbols(session)
  .find((s) => s.description === "kHandle");
const handle = session[kHandle];

// Session takes over the TCP handle.
handle.consumeStream(tcp);

// Destroying the session frees the consumed handle.
handle.destroy();

// Closing the original TCP handle must not free it a second time.
tcp.close();

console.log("ok");
