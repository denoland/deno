const tempDirPath = await Deno.makeTempDir();

const sockPath = `${tempDirPath}/control.sock`;
const testPath = `${tempDirPath}/test.ts`;

const command = new Deno.Command(Deno.execPath(), {
  env: {
    DENO_CONTROL_SOCK: sockPath,
  },
});

const child = command.spawn();

while (true) {
  try {
    await Deno.lstat(sockPath);
    break;
  } catch {
    await new Promise((r) => setTimeout(r, 10));
  }
}

const sock = await Deno.connect({
  transport: "unix",
  path: sockPath,
});

Deno.writeTextFile(
  testPath,
  `
console.log(Deno[Deno.internal].isUnconfigured());
console.log(Deno.env.get('A'));
`,
);

const encode = (s: string) => new TextEncoder().encode(s);

const data = JSON.stringify({
  cwd: [...encode(tempDirPath)],
  args: ["run", "-A", "test.ts"].map((v) => [...encode(v)]),
  env: [["A", "hello world"]].map(([k, v]) => [[...encode(k)], [...encode(v)]]),
});

await sock.write(encode(data + "\n"));

console.log(await child.status);
