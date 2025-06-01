try {
  new Deno.Command("curl", {
    env: {
      "LD_PRELOAD": "./libpreload.so",
    },
  }).spawn();
} catch (err) {
  console.log(err);
}

try {
  Deno.run({
    cmd: ["curl"],
    env: {
      "LD_PRELOAD": "./libpreload.so",
    },
  });
} catch (err) {
  console.log(err);
}
