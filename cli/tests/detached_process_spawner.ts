const process = Deno.run({
  cmd: [
    Deno.execPath(),
    "run",
    "cli/tests/detached_process_child.ts",
  ],
  stderr: "null",
  stdout: "null",
  detached: true,
});

console.log(process.pid);
