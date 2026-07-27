// Storage identity is driven by the app name, not the binary file name:
// binaries sharing an `--app-name` share a store; differing names do not.
const kv = await Deno.openKv();
const key = ["greeting"];

if (Deno.args[0] === "set") {
  await kv.set(key, "hi deno team.");
  console.log("set");
} else {
  const entry = await kv.get(key);
  console.log("value:", entry.value);
}

kv.close();
