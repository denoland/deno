let dir = await Deno.makeTempDir();

let test = await Deno.run({
  cmd: [
    Deno.execPath(),
    "test",
    "--unstable",
    "--coverage=" + dir,
    "coverage/branch_test.ts",
  ],
  stdout: "inherit",
  stderr: "inherit",
});

await test.status();

let cover = await Deno.run({
  cmd: [
    Deno.execPath(),
    "cover",
    "--unstable",
    "--lcov",
    dir,
  ],
  stdout: "inherit",
  stderr: "inherit",
});

await cover.status();
