// Regression test for https://github.com/denoland/deno/issues/24318
// When compiled with `--location`, the origin-bound storage key is derived
// from that location rather than from the main module. Two binaries with
// different names (hence different main-module URLs) compiled with the same
// `--location` therefore share the same default `Deno.openKv()` database,
// proving `--location` takes precedence over the main module.
const kv = await Deno.openKv();
const key = ["device", "key"];

if (Deno.args[0] === "set") {
  await kv.set(key, "hi deno team.");
  console.log("set");
} else {
  const { value } = await kv.get(key);
  console.log("value:", value);
}

kv.close();
