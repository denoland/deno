const output = new Deno.Command("echo", {
  env: {
    "LD_PRELOAD": "./libpreload.so",
  },
}).spawn();
