// `deno uninstall --global first second` should remove both executables from
// the bin dir in a single invocation.
const binsDir = "./bins/bin";
for await (const entry of Deno.readDir(binsDir)) {
  if (
    entry.name === "first" || entry.name === "first.cmd" ||
    entry.name === "second" || entry.name === "second.cmd"
  ) {
    throw new Error(
      `Expected ${entry.name} to be removed by 'deno uninstall --global', but it still exists`,
    );
  }
}
