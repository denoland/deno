Deno.test("output", async () => {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "console.log(0); console.error(1);"],
  });
  await p.status();
  await p.close();
  Deno.spawnSync(Deno.execPath(), {
    args: ["eval", "console.log(2); console.error(3);"],
    stdout: "inherit",
    stderr: "inherit",
  });
  await Deno.spawn(Deno.execPath(), {
    args: ["eval", "console.log(4); console.error(5);"],
    stdout: "inherit",
    stderr: "inherit",
  });
  const c = await Deno.spawnChild(Deno.execPath(), {
    args: ["eval", "console.log(6); console.error(7);"],
    stdout: "inherit",
    stderr: "inherit",
  });
  await c.status;
  const worker = new Worker(
    new URL("./captured_output.worker.js", import.meta.url).href,
    { type: "module" },
  );

  // ensure worker output is captured
  const response = new Promise<void>((resolve) =>
    worker.onmessage = () => resolve()
  );
  worker.postMessage({});
  await response;
  worker.terminate();
});
