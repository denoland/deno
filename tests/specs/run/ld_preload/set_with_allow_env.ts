Deno.env.set("LD_PRELOAD", "./libpreload.so");

const output = new Deno.Command("echo").spawn();
