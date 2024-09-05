// Testing the following (but with `deno` instead of `echo`):
// | `deno run --allow-run=echo`         | `which path == "/usr/bin/echo"` at startup | `which path != "/usr/bin/echo"` at startup |
// |-------------------------------------|--------------------------------------------|--------------------------------------------|
// | **`Deno.Command("echo")`**          | ✅                                          | ✅                                         |
// | **`Deno.Command("/usr/bin/echo")`** | ✅                                          | ❌                                         |

// | `deno run --allow-run=/usr/bin/echo | `which path == "/usr/bin/echo"` at runtime | `which path != "/usr/bin/echo"` at runtime |
// |-------------------------------------|--------------------------------------------|--------------------------------------------|
// | **`Deno.Command("echo")`**          | ✅                                          | ❌                                         |
// | **`Deno.Command("/usr/bin/echo")`** | ✅                                          | ✅                                         |

const execPath = Deno.execPath();
const execPathParent = execPath.replace(/[/\\][^/\\]+$/, "");

const testUrl = `data:application/typescript;base64,${
  btoa(`
  console.error(await Deno.permissions.query({ name: "run", command: "deno" }));
  console.error(await Deno.permissions.query({ name: "run", command: "${
    execPath.replaceAll("\\", "\\\\")
  }" }));
  Deno.env.set("PATH", "");
  console.error(await Deno.permissions.query({ name: "run", command: "deno" }));
  console.error(await Deno.permissions.query({ name: "run", command: "${
    execPath.replaceAll("\\", "\\\\")
  }" }));
`)
}`;

const process1 = await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-env",
    "--allow-run=deno",
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
    "--allow-run=deno",
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
