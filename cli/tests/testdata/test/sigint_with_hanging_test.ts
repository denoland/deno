const process = new Deno.Command(Deno.execPath(), {
  args: [
    "test",
    "--quiet",
    "--no-check",
    new URL("hanging_test.ts", import.meta.url).href,
  ],
  stdout: "inherit",
  stderr: "inherit",
}).spawn();
await new Promise((r) => setTimeout(r, 1000));
process.kill("SIGINT");
const output = await process.output();
console.assert(output.code == 130, "Exit code should be 130");
