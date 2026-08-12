// Runs the OOM script as a child process so this parent can observe the child
// dying from a V8 fatal OOM (a signal, which the spec harness can't assert on
// directly) while itself exiting cleanly. The child writes the `.heapsnapshot`
// into the shared cwd, which the next step asserts on.
const cmd = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-write=.",
    "--v8-flags=--max-old-space-size=20",
    "oom.mjs",
  ],
  stdout: "null",
  stderr: "piped",
});
const { success, stderr } = await cmd.output();
const text = new TextDecoder().decode(stderr);

if (!text.includes("Writing heap snapshot to ")) {
  console.error("child did not write a heap snapshot");
  console.error(text);
  Deno.exit(1);
}
if (success) {
  console.error("child unexpectedly exited successfully");
  Deno.exit(1);
}
console.log("oom-child-terminated");
