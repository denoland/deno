Deno.env.set("LD_PRELOAD", "./libpreload.so");

try {
  new Deno.Command("echo").spawn();
} catch (err) {
  console.log(err);
}

Deno.env.set("DYLD_FALLBACK_LIBRARY_PATH", "./libpreload.so");

try {
  Deno.run({ cmd: ["echo"] }).spawnSync();
} catch (err) {
  console.log(err);
}
