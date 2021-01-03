Deno.test("spawn test", async function () {
  const process = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-all",
      "--unstable",
      "run_coverage.ts",
    ],
  });

  await process.status();
  process.close();
});
