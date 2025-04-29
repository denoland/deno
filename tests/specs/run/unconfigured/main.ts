const tempDirPath = await Deno.makeTempDir();

const sockPath = `${tempDirPath}/control.sock`;
const testPath = `${tempDirPath}/test.ts`;

const command = new Deno.Command(Deno.execPath(), {
  env: {
    DENO_UNSTABLE_CONTROL_SOCK: `unix:${sockPath}`,
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

const data = JSON.stringify({
  cwd: tempDirPath,
  args: ["run", "-A", "test.ts"],
  env: [["A", "hello world"]],
});

await sock.write(new TextEncoder().encode(data + "\n"));

console.log(await child.status);
