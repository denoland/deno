const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";
Deno.copyFileSync(Deno.execPath(), binaryName);

console.log("Running...");
new Deno.Command(
  Deno.execPath(),
  {
    args: [
      "run",
      "--allow-write",
      "--allow-read",
      `--allow-run=binary`,
      "sub.ts",
    ],
    env: {
      PATH: Deno.cwd(),
    },
    stderr: "inherit",
    stdout: "inherit",
  },
).outputSync();
