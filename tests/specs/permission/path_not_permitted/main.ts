const binaryName = Deno.build.os === "windows" ? "deno.exe" : "deno";
Deno.copyFileSync(Deno.execPath(), binaryName);

console.log("Running...");
new Deno.Command(
  Deno.execPath(),
  {
    args: [
      "run",
      "--allow-write",
      "--allow-read",
      `--allow-run=deno`,
      "sub.ts",
    ],
    stderr: "inherit",
    stdout: "inherit",
  },
).outputSync();
