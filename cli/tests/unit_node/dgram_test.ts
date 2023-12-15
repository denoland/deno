import {assertEquals, execCode2} from "../unit/test_util";

const listenPort = 4503;
const listenPort2 = 4504;

Deno.test(
  { permissions: { read: true, run: true, net: true } },
  async function dgramUdpListenUnrefAndRef() {
    const p = execCode2(`
      import * as dgram from "node:dgram";
      async function main() {
        const udpSocket = dgram.createSocket('udp4');
        udpSocket.bind(${listenPort});
        listener.unref();
        listener.ref(); // This restores 'ref' state of listener
        console.log("started");
        udpSocket.on('message', (buffer, rinfo) => {
          console.log("accepted");
        }
      }
      main();
    `);
    await p.waitStdoutText("started");
    const conn = await Deno.connect({ port: listenPort });
    conn.send("Deno!");
    conn.close();
    const [statusCode, output] = await p.finished();
    assertEquals(statusCode, 0);
    assertEquals(output.trim(), "started\naccepted");
  },
);

Deno.test(
  { permissions: { read: true, run: true, net: true } },
  async function dgramUdpListenUnref() {
    const p = execCode2(`
      import * as dgram from "node:dgram";
      async function main() {
        const udpSocket = dgram.createSocket('udp4');
        udpSocket.bind(${listenPort});
        listener.unref();
        udpSocket.on('message', (buffer, rinfo) => {
          console.log("accepted");
        }
        console.log("started");
      }
      main();
    `);
    await p.waitStdoutText("started");
    const conn = await Deno.connect({ port: listenPort });
    conn.send("Deno!");
    conn.close();
    const [statusCode, output] = await p.finished();
    assertEquals(statusCode, 0);
    assertEquals(output.trim(), "started");
  },
);
