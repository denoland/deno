// Tiny JSR script used by `deno x` spec tests to verify that
// `--unstable-*` flags are forwarded into the spawned `deno run` invocation.
const kv = await Deno.openKv(":memory:");
await kv.set(["x"], 42);
const entry = await kv.get(["x"]);
console.log("ok:", entry.value);
kv.close();
