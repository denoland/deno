const cmd = new Deno.Command(Deno.execPath(), {
  args: ["run", "--env-file=.env", "-A", "./spawn.ts"],
  env: {
    FOO: "overridden_by_process_level_env",
  },
  stdout: "piped",
  stderr: "piped",
});

const child = cmd.spawn();
const { code, stderr, stdout } = await child.output();
const decodedStderr = new TextDecoder().decode(stderr);
const decodedStdout = new TextDecoder().decode(stdout);

if (code !== 0) {
  console.error("Command failed with code " + code);
  console.error("stderr: " + decodedStderr);
  Deno.exit(code);
}

console.log(decodedStdout);
