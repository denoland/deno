Deno.test("spawn test", async function () {
  console.log("SPAWNING PROCESS");
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
