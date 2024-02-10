const child = new Deno.Command("cat", {
  args: ["-"],
  stdout: "null",
  stderr: "null",
}).spawn();
child.kill("SIGTERM");
