const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";
Deno.copyFileSync(Deno.execPath(), binaryName);

console.log("Running...");
const result = new Deno.Command(
  Deno.execPath(),
  {
    args: ["run", "--allow-write", `--allow-run=./${binaryName}`, "sub.ts"],
    stderr: "inherit",
    stdout: "inherit",
  },
).outputSync();

console.assert(result.code == 1, "Expected failure");
