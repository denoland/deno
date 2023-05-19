const child = new Deno.Command(Deno.execPath(), {
  args: ["eval", "await new Promise(r => setTimeout(r, 2000))"],
  stdout: "null",
  stderr: "null",
}).spawn();
child.kill("SIGTERM");
