const child = new Deno.Command("deno", {
  args: ["eval", "await new Promise(r => setTimeout(r, 2000))"],
  stdout: "null",
  stderr: "null",
}).spawn();
child.kill("SIGTERM");
