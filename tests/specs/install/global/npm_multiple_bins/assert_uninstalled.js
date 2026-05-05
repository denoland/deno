// Assert that both bin entries were removed after uninstall
const binsDir = "./bins/bin";
const entries = [];
for await (const entry of Deno.readDir(binsDir)) {
  entries.push(entry.name);
}

// After uninstall, multi-bin and multi-bin-server should be gone
for (const name of entries) {
  if (
    name === "multi-bin" || name === "multi-bin.cmd" ||
    name === "multi-bin-server" || name === "multi-bin-server.cmd"
  ) {
    throw new Error(
      `Expected ${name} to be removed after uninstall, but it still exists`,
    );
  }
}
