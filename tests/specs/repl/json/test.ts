import { Socket } from "node:net";

const {
  kExtraStdio,
  getExtraPipeFds,
} = Deno[Deno.internal];

const command = new Deno.Command(Deno.execPath(), {
  args: [
    "repl",
    "--json",
  ],
  stdio: "null",
  stderr: "inherit",
  stdout: "inherit",
  [kExtraStdio]: ["piped"],
});

await using child = command.spawn();

const pipeFd = getExtraPipeFds(child)[0];
const socket = new Socket({ fd: pipeFd });

function writeMessage(socket: Socket, msg: object): Promise<void> {
  return new Promise((resolve, reject) => {
    const buf = new TextEncoder().encode(JSON.stringify(msg));
    const header = new Uint8Array(4);
    new DataView(header.buffer).setUint32(0, buf.length, true);
    socket.write(header, (err) => {
      if (err) return reject(err);
      socket.write(buf, (err) => {
        if (err) return reject(err);
        resolve();
      });
    });
  });
}

function readMessage(socket: Socket): Promise<unknown> {
  return new Promise((resolve, reject) => {
    let headerBuf = Buffer.alloc(0);
    let bodyBuf = Buffer.alloc(0);
    let bodyLen: number | null = null;

    const onData = (chunk: Buffer) => {
      let data = chunk;
      // Read 4-byte length header
      if (bodyLen === null) {
        headerBuf = Buffer.concat([headerBuf, data]);
        if (headerBuf.length < 4) return;
        bodyLen = headerBuf.readUInt32LE(0);
        data = headerBuf.subarray(4);
      }
      // Accumulate body
      bodyBuf = Buffer.concat([bodyBuf, data]);
      if (bodyBuf.length >= bodyLen) {
        socket.removeListener("data", onData);
        socket.removeListener("error", onError);
        resolve(JSON.parse(bodyBuf.subarray(0, bodyLen).toString()));
      }
    };
    const onError = (err: Error) => {
      socket.removeListener("data", onData);
      reject(err);
    };
    socket.on("data", onData);
    socket.on("error", onError);
  });
}

await writeMessage(socket, { type: "Run", code: "let a = 1;", output: false });
console.log(await readMessage(socket));

await writeMessage(socket, {
  type: "Run",
  code: "console.log('hello'); a + 1",
  output: true,
});
console.log(await readMessage(socket));

await writeMessage(socket, {
  type: "Run",
  code: "throw new Error('hi')",
  output: true,
});
console.log(await readMessage(socket));

// Test EOF (ctrl+d): closing the underlying socket should cause a clean exit
socket.destroy();
const status = await child.status;
console.log(status.code);
