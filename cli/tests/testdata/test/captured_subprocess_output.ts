Deno.test("output", async () => {
  const p = Deno.run({
    cmd: [Deno.exePath(), "eval", "console.log(1); console.error(2);"],
  });
  await p.status();
  await p.close();
  Deno.spawnSync(Deno.exePath(), {
    args: ["eval", "console.log(3); console.error(4);"],
    stdout: "inherit",
    stderr: "inherit",
  });
  await Deno.spawn(Deno.exePath(), {
    args: ["eval", "console.log(5); console.error(6);"],
    stdout: "inherit",
    stderr: "inherit",
  });
  const c = await Deno.spawnChild(Deno.exePath(), {
    args: ["eval", "console.log(7); console.error(8);"],
    stdout: "inherit",
    stderr: "inherit",
  });
  await c.status;
});
