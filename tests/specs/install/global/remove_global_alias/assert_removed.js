// `deno remove --global` is an alias for `deno uninstall --global`, so after
// it runs the globally installed executable should be gone from the bin dir.
const binsDir = "./bins/bin";
for await (const entry of Deno.readDir(binsDir)) {
  if (entry.name === "deno-test-bin" || entry.name === "deno-test-bin.cmd") {
    throw new Error(
      `Expected ${entry.name} to be removed by 'deno remove --global', but it still exists`,
    );
  }
}
