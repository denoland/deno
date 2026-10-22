const run = async (cmd: string[]) => {
  const c = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    stdout: "inherit",
    stderr: "inherit",
  });
  const { success } = await c.output();
  if (!success) throw new Error(`Command failed: ${cmd.join(" ")}`);
};

await run(["git", "init"]);
await run(["git", "config", "user.email", "test@test.com"]);
await run(["git", "config", "user.name", "Test"]);
await run(["git", "add", "."]);
await run(["git", "commit", "-m", "feat(a): initial feature"]);
