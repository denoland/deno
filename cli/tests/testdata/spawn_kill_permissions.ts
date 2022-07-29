const child = Deno.spawnChild("deno", {
  args: ["eval", "await new Promise(r => setTimeout(r, 2000))"],
  stdout: "null",
  stderr: "null",
});
child.kill("SIGTERM");
