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
