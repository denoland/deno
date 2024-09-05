try {
  new Deno.Command("echo", {
    env: {
      "LD_PRELOAD": "./libpreload.so",
    },
  }).spawn();
} catch (err) {
  console.log(err);
}

try {
  Deno.run({
    cmd: ["echo"],
    env: {
      "LD_PRELOAD": "./libpreload.so",
    },
  });
} catch (err) {
  console.log(err);
}
