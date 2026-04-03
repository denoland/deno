// Test that TCP.open(fd) works with real TCP socket fds.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

const { TCP, constants: TCPConstants } = require("internal/test/binding")
  .internalBinding("tcp_wrap");

// Test 1: TCP.open with invalid fd returns error code (not notImplemented)
{
  const tcp = new TCP(TCPConstants.SOCKET);
  const err = tcp.open(-1);
  if (err !== 0) {
    console.log("PASS: TCP.open(-1) returned error");
  } else {
    console.log("FAIL: TCP.open(-1) should return error");
  }
}

// Test 2: TCP.open with a real connected TCP socket fd
const net = require("net");

const server = net.createServer((conn: any) => {
  conn.write("hello via tcp.open");
  conn.end();
});

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address();

  // Create a raw TCP socket and connect it using FFI
  const libName = Deno.build.os === "darwin"
    ? "libSystem.B.dylib"
    : Deno.build.os === "windows"
    ? "ws2_32.dll"
    : "libc.so.6";
  const lib = Deno.dlopen(libName, {
    socket: { parameters: ["i32", "i32", "i32"], result: "i32" },
    connect: { parameters: ["i32", "buffer", "i32"], result: "i32" },
    close: { parameters: ["i32"], result: "i32" },
  });

  const AF_INET = 2;
  const SOCK_STREAM = 1;

  // Create TCP socket
  const sockFd = lib.symbols.socket(AF_INET, SOCK_STREAM, 0);
  if (sockFd < 0) {
    console.log("FAIL: socket() failed");
    server.close();
    lib.close();
    Deno.exit(1);
  }

  // Build sockaddr_in for 127.0.0.1:port
  const addr = new Uint8Array(16);
  const view = new DataView(addr.buffer);
  // sin_family = AF_INET (little-endian on most platforms, but sa_family_t
  // layout varies). Use platform-appropriate offset.
  if (Deno.build.os === "darwin") {
    addr[0] = 16; // sin_len (BSD)
    addr[1] = AF_INET; // sin_family
  } else {
    view.setUint16(0, AF_INET, true); // sin_family
  }
  view.setUint16(2, port, false); // sin_port (network byte order)
  // 127.0.0.1
  addr[4] = 127;
  addr[5] = 0;
  addr[6] = 0;
  addr[7] = 1;

  const connectResult = lib.symbols.connect(sockFd, addr, 16);
  if (connectResult < 0) {
    console.log("FAIL: connect() failed");
    lib.symbols.close(sockFd);
    server.close();
    lib.close();
    Deno.exit(1);
  }

  // Now open this raw TCP socket fd in a TCP handle
  const tcp = new TCP(TCPConstants.SOCKET);
  const openErr = tcp.open(sockFd);
  if (openErr !== 0) {
    console.log("FAIL: TCP.open(fd) returned error:", openErr);
    lib.symbols.close(sockFd);
    server.close();
    lib.close();
    Deno.exit(1);
  }

  // Wrap in a net.Socket and read data
  const socket = new net.Socket({ handle: tcp });
  let data = "";
  socket.setEncoding("utf8");
  socket.on("data", (chunk: string) => {
    data += chunk;
  });
  socket.on("end", () => {
    console.log("PASS: received:", data);
    socket.destroy();
    server.close();
    lib.close();
  });
  socket.on("error", (err: any) => {
    console.log("FAIL: socket error:", err.message);
    server.close();
    lib.close();
  });
  socket.resume();
});
