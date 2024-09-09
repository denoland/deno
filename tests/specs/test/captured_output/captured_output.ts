Deno.test("output", async () => {
  await new Deno.Command(Deno.execPath(), {
    args: ["eval", "console.log(0); console.error(1);"],
  }).spawn().status;
  new Deno.Command(Deno.execPath(), {
    args: ["eval", "console.log(2); console.error(3);"],
    stdout: "inherit",
    stderr: "inherit",
  }).outputSync();
  await new Deno.Command(Deno.execPath(), {
    args: ["eval", "console.log(4); console.error(5);"],
    stdout: "inherit",
    stderr: "inherit",
  }).output();
  const c = new Deno.Command(Deno.execPath(), {
    args: ["eval", "console.log(6); console.error(7);"],
    stdout: "inherit",
    stderr: "inherit",
  }).spawn();
  await c.status;
  const worker = new Worker(
    import.meta.resolve("./captured_output.worker.ts"),
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
