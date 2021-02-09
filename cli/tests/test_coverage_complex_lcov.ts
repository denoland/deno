const dir = await Deno.makeTempDir();

const test = await Deno.run({
  cmd: [
    Deno.execPath(),
    "test",
    "--unstable",
    "--coverage=" + dir,
    "coverage/complex_test.ts",
  ],
  stdout: "inherit",
  stderr: "inherit",
});

await test.status();

const coverage = await Deno.run({
  cmd: [
    Deno.execPath(),
    "coverage",
    "--unstable",
    "--lcov",
    dir,
  ],
  stdout: "inherit",
  stderr: "inherit",
});

await coverage.status();
