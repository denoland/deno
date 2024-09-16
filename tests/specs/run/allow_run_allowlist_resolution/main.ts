// Testing the following:
// | `deno run --allow-run=binary`         | `which path == "/usr/bin/binary"` at startup | `which path != "/usr/bin/binary"` at startup |
// |---------------------------------------|----------------------------------------------|--------------------------------------------|
// | **`Deno.Command("binary")`**          | :white_check_mark:                                          | :white_check_mark:                                         |
// | **`Deno.Command("/usr/bin/binary")`** | :white_check_mark:                                          | :x:                                         |
// | `deno run --allow-run=/usr/bin/binary | `which path == "/usr/bin/binary"` at runtime | `which path != "/usr/bin/binary"` at runtime |
// |---------------------------------------|----------------------------------------------|--------------------------------------------|
// | **`Deno.Command("binary")`**          | :white_check_mark:                                          | :x:                                         |
// | **`Deno.Command("/usr/bin/binary")`** | :white_check_mark:                                          | :white_check_mark:                                         |

const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";
const pathSep = Deno.build.os === "windows" ? "\\" : "/";
const cwd = Deno.cwd();
const execPathParent = `${Deno.cwd()}${pathSep}sub`;
const execPath = `${execPathParent}${pathSep}${binaryName}`;

Deno.mkdirSync(execPathParent);
Deno.copyFileSync(Deno.execPath(), execPath);

const testUrl = `data:application/typescript;base64,${
  btoa(`
  console.error(await Deno.permissions.query({ name: "run", command: "binary" }));
  console.error(await Deno.permissions.query({ name: "run", command: "${
    execPath.replaceAll("\\", "\\\\")
  }" }));
  Deno.env.set("PATH", "");
  console.error(await Deno.permissions.query({ name: "run", command: "binary" }));
  console.error(await Deno.permissions.query({ name: "run", command: "${
    execPath.replaceAll("\\", "\\\\")
  }" }));
`)
}`;

await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-env",
    "--allow-run=binary",
    testUrl,
  ],
  stdout: "inherit",
  stderr: "inherit",
  env: { "PATH": execPathParent },
}).output();

console.error("---");

await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-env",
    "--allow-run=binary",
    testUrl,
  ],
  stderr: "inherit",
  stdout: "inherit",
  env: { "PATH": "" },
}).output();

console.error("---");

await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-env",
    `--allow-run=${execPath}`,
    testUrl,
  ],
  stderr: "inherit",
  stdout: "inherit",
  env: { "PATH": execPathParent },
}).output();
