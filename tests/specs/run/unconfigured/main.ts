const tempDirPath = await Deno.makeTempDir();

const sockPath = `${tempDirPath}/control.sock`;
const testPath = `${tempDirPath}/test.ts`;

const command = new Deno.Command(Deno.execPath(), {
  env: {
    DENO_UNSTABLE_CONTROL_SOCK: `unix:${sockPath}`,
  },
});

const child = command.spawn();

let i = 0;
while (true) {
  try {
    await Deno.lstat(sockPath);
    break;
  } catch {}

  i += 1;
  if (i > 100) {
    throw new Error(`${sockPath} did not exist`);
  }

  await new Promise((r) => setTimeout(r, 10));
}

const sock = await Deno.connect({
  transport: "unix",
  path: sockPath,
});

Deno.writeTextFileSync(
  testPath,
  `
console.log(Deno[Deno.internal].isFromUnconfiguredRuntime());
console.log(Deno.env.get('A'));
Deno.serve({ onListen() {} }, () => {}).shutdown();
`,
);

const data = JSON.stringify({
  cwd: tempDirPath,
  args: ["run", "-A", "test.ts"],
  env: [["A", "hello world"]],
});

await sock.write(new TextEncoder().encode(data + "\n"));

const buf = new Uint8Array(128);
const n = await sock.read(buf);

console.log("EVENT:", new TextDecoder().decode(buf.subarray(0, n)));

console.log(await child.status);

const nodeSockPath = `${tempDirPath}/node-control.sock`;
const nodeOverrideSockPath = `${tempDirPath}/node-override.sock`;
const nodeTestPath = `${tempDirPath}/node_test.ts`;

const nodeCommand = new Deno.Command(Deno.execPath(), {
  env: {
    DENO_UNSTABLE_CONTROL_SOCK: `unix:${nodeSockPath}`,
  },
});

const nodeChild = nodeCommand.spawn();

i = 0;
while (true) {
  try {
    await Deno.lstat(nodeSockPath);
    break;
  } catch {}

  i += 1;
  if (i > 100) {
    throw new Error(`${nodeSockPath} did not exist`);
  }

  await new Promise((r) => setTimeout(r, 10));
}

const nodeSock = await Deno.connect({
  transport: "unix",
  path: nodeSockPath,
});

Deno.writeTextFileSync(
  nodeTestPath,
  `
import http from "node:http";
const server = http.createServer((_req, res) => res.end("ok"));
server.listen(0, () => {
  console.log("node listening");
  server.close();
});
`,
);

const nodeData = JSON.stringify({
  cwd: tempDirPath,
  args: ["run", "-A", "node_test.ts"],
  env: [
    ["DENO_AUTO_SERVE", "1"],
    ["DENO_SERVE_ADDRESS", `duplicate,unix:${nodeOverrideSockPath}`],
  ],
});

await nodeSock.write(new TextEncoder().encode(nodeData + "\n"));

const nodeBuf = new Uint8Array(128);
const nodeN = await nodeSock.read(nodeBuf);

console.log(
  "NODE EVENT:",
  new TextDecoder().decode(nodeBuf.subarray(0, nodeN)),
);

console.log(await nodeChild.status);

const h2SockPath = `${tempDirPath}/h2-control.sock`;
const h2OverrideSockPath = `${tempDirPath}/h2-override.sock`;
const h2TestPath = `${tempDirPath}/h2_test.ts`;

const h2Command = new Deno.Command(Deno.execPath(), {
  env: {
    DENO_UNSTABLE_CONTROL_SOCK: `unix:${h2SockPath}`,
  },
});

const h2Child = h2Command.spawn();

i = 0;
while (true) {
  try {
    await Deno.lstat(h2SockPath);
    break;
  } catch {}

  i += 1;
  if (i > 100) {
    throw new Error(`${h2SockPath} did not exist`);
  }

  await new Promise((r) => setTimeout(r, 10));
}

const h2Sock = await Deno.connect({
  transport: "unix",
  path: h2SockPath,
});

Deno.writeTextFileSync(
  h2TestPath,
  `
import http2 from "node:http2";
const server = http2.createServer();
server.listen(0, () => {
  console.log("http2 listening");
  server.close();
});
`,
);

const h2Data = JSON.stringify({
  cwd: tempDirPath,
  args: ["run", "-A", "h2_test.ts"],
  env: [
    ["DENO_AUTO_SERVE", "1"],
    ["DENO_SERVE_ADDRESS", `duplicate,unix:${h2OverrideSockPath}`],
  ],
});

await h2Sock.write(new TextEncoder().encode(h2Data + "\n"));

const h2Buf = new Uint8Array(128);
const h2N = await h2Sock.read(h2Buf);

console.log(
  "HTTP2 EVENT:",
  new TextDecoder().decode(h2Buf.subarray(0, h2N)),
);

console.log(await h2Child.status);
