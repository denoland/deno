// Testing the following (but with `deno` instead of `echo`):
// | `deno run --allow-run=echo`         | `which path == "/usr/bin/echo"` at startup | `which path != "/usr/bin/echo"` at startup |
// |-------------------------------------|--------------------------------------------|--------------------------------------------|
// | **`Deno.Command("echo")`**          | ✅                                          | ✅                                          |
// | **`Deno.Command("/usr/bin/echo")`** | ✅                                          | ❌                                          |

// | `deno run --allow-run=/usr/bin/echo | `which path == "/usr/bin/echo"` at runtime | `which path != "/usr/bin/echo"` at runtime |
// |-------------------------------------|--------------------------------------------|--------------------------------------------|
// | **`Deno.Command("echo")`**          | ✅                                          | ❌                                          |
// | **`Deno.Command("/usr/bin/echo")`** | ✅                                          | ✅                                          |

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

const process1 = await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--allow-env",
    "--allow-run=deno",
    testUrl,
  ],
  stderr: "null",
  env: { "PATH": execPathParent },
}).output();
console.log(new TextDecoder().decode(process1.stdout));

const process2 = await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--allow-env",
    "--allow-run=deno",
    testUrl,
  ],
  stderr: "null",
  env: { "PATH": "" },
}).output();
console.log(new TextDecoder().decode(process2.stdout));

const process3 = await new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--allow-env",
    `--allow-run=${execPath}`,
    testUrl,
  ],
  stderr: "null",
  env: { "PATH": execPathParent },
}).output();
console.log(new TextDecoder().decode(process3.stdout));
