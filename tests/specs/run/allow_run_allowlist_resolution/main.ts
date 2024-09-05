// Testing the following:
// | `deno run --allow-run=binary`         | `which path == "/usr/bin/binary"` at startup | `which path != "/usr/bin/binary"` at startup |
// |---------------------------------------|----------------------------------------------|--------------------------------------------|
// | **`Deno.Command("binary")`**          | ✅                                          | ✅                                         |
// | **`Deno.Command("/usr/bin/binary")`** | ✅                                          | ❌                                         |

// | `deno run --allow-run=/usr/bin/binary | `which path == "/usr/bin/binary"` at runtime | `which path != "/usr/bin/binary"` at runtime |
// |---------------------------------------|----------------------------------------------|--------------------------------------------|
// | **`Deno.Command("binary")`**          | ✅                                          | ❌                                         |
// | **`Deno.Command("/usr/bin/binary")`** | ✅                                          | ✅                                         |

const binaryName = Deno.build.os === "windows" ? "binary.exe" : "binary";
const pathSep = Deno.build.os === "windows" ? "\\" : "/";
const cwd = Deno.cwd();
const execPathParent = `${Deno.cwd()}${pathSep}sub`;
const execPath = `${execPathParent}${pathSep}${binaryName}`;
const execPathSymlinkParent = `${Deno.cwd()}${pathSep}link`;
const execPathSymlink = `${execPathSymlinkParent}${pathSep}${binaryName}`;
Deno.mkdirSync(execPathParent);
Deno.mkdirSync(execPathSymlinkParent);
Deno.copyFileSync(Deno.execPath(), execPath);
Deno.symlinkSync(execPath, execPathSymlink, { type: "file" });

const fileText = `
console.error(await Deno.permissions.query({ name: "run", command: "binary" }));
console.error(await Deno.permissions.query({ name: "run", command: "${
  execPath.replaceAll("\\", "\\\\")
}" }));
Deno.env.set("PATH", "");
console.error(await Deno.permissions.query({ name: "run", command: "binary" }));
console.error(await Deno.permissions.query({ name: "run", command: "${
  execPath.replaceAll("\\", "\\\\")
}" }));
console.error(await Deno.permissions.query({ name: "run", command: "${
  execPathSymlink.replaceAll("\\", "\\\\")
}" }));
`;
const testUrl = `data:application/typescript;base64,${btoa(fileText)}`;

console.error("--- name, on PATH ---");

const process1 = await new Deno.Command(Deno.execPath(), {
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

console.error("--- name, no PATH ---");

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

console.error("--- path, on PATH ---");

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

console.error("--- path, not on PATH (same as above) ---");

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

console.error("--- path, dir symlink on PATH (same as above) ---");

await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-env",
    `--allow-run=${execPath}`,
    testUrl,
  ],
  stderr: "inherit",
  stdout: "inherit",
  env: { "PATH": execPathSymlinkParent },
}).output();

console.error("--- symlink path, dir symlink on PATH (same as above) ---");

await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-env",
    `--allow-run=${execPathSymlink}`,
    testUrl,
  ],
  stderr: "inherit",
  stdout: "inherit",
  env: { "PATH": execPathSymlinkParent },
}).output();
