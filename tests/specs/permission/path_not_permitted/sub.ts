const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";
const pathSep = Deno.build.os === "windows" ? "\\" : "/";

Deno.mkdirSync("subdir");
Deno.copyFileSync(binaryName, "subdir/" + binaryName);

const { code, stdout, stderr } = new Deno.Command(
  binaryName,
  {
    env: { "PATH": Deno.cwd() + pathSep + "subdir" },
    stdout: "inherit",
    stderr: "inherit",
  },
).outputSync();

console.log(code);
