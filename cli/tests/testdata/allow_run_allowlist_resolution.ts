// Testing the following (but with `deno` instead of `echo`):
// | `deno run --allow-run=echo`              | `which path == "/usr/bin/echo"` at startup | `which path != "/usr/bin/echo"` at startup |
// |------------------------------------------|--------------------------------------------|--------------------------------------------|
// | **`Deno.run({ cmd: "echo" })`**          | ✅                                          | ✅                                          |
// | **`Deno.run({ cmd: "/usr/bin/echo" })`** | ✅                                          | ❌                                          |

// | `deno run --allow-run=/usr/bin/echo`     | `which path == "/usr/bin/echo"` at runtime | `which path != "/usr/bin/echo"` at runtime |
// |------------------------------------------|--------------------------------------------|--------------------------------------------|
// | **`Deno.run({ cmd: "echo" })`**          | ✅                                          | ❌                                          |
// | **`Deno.run({ cmd: "/usr/bin/echo" })`** | ✅                                          | ✅                                          |

const execPath = Deno.execPath();
const execPathParent = execPath.replace(/[/\\][^/\\]+$/, "");

const testUrl = `data:application/typescript;base64,${
  btoa(`
  console.log(await Deno.permissions.query({ name: "run", command: "deno" }));
  console.log(await Deno.permissions.query({ name: "run", command: "${
    execPath.replaceAll("\\", "\\\\")
  }" }));
  Deno.env.set("PATH", "");
  console.log(await Deno.permissions.query({ name: "run", command: "deno" }));
  console.log(await Deno.permissions.query({ name: "run", command: "${
    execPath.replaceAll("\\", "\\\\")
  }" }));
`)
}`;

const process1 = Deno.run({
  cmd: [
    execPath,
    "run",
    "--quiet",
    "--allow-env",
    "--allow-run=deno",
    testUrl,
  ],
  stdout: "piped",
  env: { "PATH": execPathParent },
});
console.log(new TextDecoder().decode(await process1.output()));

const process2 = Deno.run({
  cmd: [
    execPath,
    "run",
    "--quiet",
    "--allow-env",
    "--allow-run=deno",
    testUrl,
  ],
  stdout: "piped",
  env: { "PATH": "" },
});
console.log(new TextDecoder().decode(await process2.output()));

const process3 = Deno.run({
  cmd: [
    execPath,
    "run",
    "--quiet",
    "--allow-env",
    `--allow-run=${execPath}`,
    testUrl,
  ],
  stdout: "piped",
  env: { "PATH": execPathParent },
});
console.log(new TextDecoder().decode(await process3.output()));
