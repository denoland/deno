await new Deno.Command(Deno.execPath(), {
  args: ["eval", "--quiet", "console.log('Hello, world! 1')"],
  stdout: "inherit",
}).output();
new Deno.Command(Deno.execPath(), {
  args: ["eval", "--quiet", "console.log('Hello, world! 2')"],
  stdout: "inherit",
}).outputSync();
