await Deno.spawn(Deno.execPath(), {
  args: ["eval", "--quiet", "console.log('Hello, world! 1')"],
  stdout: "inherit",
});
Deno.spawnSync(Deno.execPath(), {
  args: ["eval", "--quiet", "console.log('Hello, world! 2')"],
  stdout: "inherit",
});
