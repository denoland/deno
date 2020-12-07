Deno.test("spawn test", async function () {
  const process = Deno.run({
    cmd: [
      Deno.execPath(),
      "test",
      "--allow-all",
      "--unstable",
      "test_coverage.ts",
    ],
  });

  await process.status();
  process.close();
});
