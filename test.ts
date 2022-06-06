const child = Deno.spawnChild(Deno.execPath(), {
  args: [
    "eval",
    "--unstable",
    "setTimeout(() => console.log(1), 3000);",
  ],
  stdout: "inherit",
  stderr: "inherit",
});
console.log(0);
child.unref();
child.status.then(() => console.log(2));
