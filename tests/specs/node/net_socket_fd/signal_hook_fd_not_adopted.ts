const childPath = new URL(
  "./signal_hook_fd_not_adopted_child.ts",
  import.meta.url,
);

const { code, stderr, success } = await new Deno.Command(Deno.execPath(), {
  args: ["run", "-A", childPath.pathname],
  stdout: "piped",
  stderr: "piped",
}).output();

const stderrText = new TextDecoder().decode(stderr);
if (
  stderrText.includes("Deno has panicked") ||
  stderrText.includes("panicked at") ||
  stderrText.includes("fatal runtime error")
) {
  console.error(stderrText);
  Deno.exit(1);
}

if (!success) {
  console.error(stderrText);
  Deno.exit(code);
}

console.log("done");
