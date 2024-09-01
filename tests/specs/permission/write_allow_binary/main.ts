const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";
Deno.copyFileSync(Deno.execPath(), binaryName);

const result = new Deno.Command(
  "deno",
  {
    args: ["run", "--allow-write", `--allow-run=./${binaryName}`, "sub.ts"],
    stderr: "inherit",
    stdout: "inherit",
  },
).outputSync();

console.log(result);
