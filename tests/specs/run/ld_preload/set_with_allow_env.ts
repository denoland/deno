Deno.env.set("LD_PRELOAD", "./libpreload.so");

try {
  new Deno.Command("curl").spawn();
} catch (err) {
  console.log(err);
}

Deno.env.set("DYLD_FALLBACK_LIBRARY_PATH", "./libpreload.so");

try {
  Deno.run({ cmd: ["curl"] }).spawnSync();
} catch (err) {
  console.log(err);
}
