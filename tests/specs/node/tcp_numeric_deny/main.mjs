// Verify that numeric hostname aliases (decimal/hex representations of
// 127.0.0.1) are denied by --deny-net after resolution, matching the
// behavior of native Deno.connect().

import net from "node:net";

const port = 12345;

function tryConnect(host) {
  return new Promise((resolve) => {
    const sock = net.connect({ host, port }, () => {
      sock.destroy();
      resolve("OK");
    });
    sock.on("error", (err) => {
      resolve(err.code || err.message);
    });
  });
}

// 2130706433 = 127.0.0.1 in decimal
// 0x7f000001 = 127.0.0.1 in hex
const decimal = await tryConnect("2130706433");
const hex = await tryConnect("0x7f000001");
const literal = await tryConnect("127.0.0.1");

console.log(`decimal:${decimal}`);
console.log(`hex:${hex}`);
console.log(`literal:${literal}`);
