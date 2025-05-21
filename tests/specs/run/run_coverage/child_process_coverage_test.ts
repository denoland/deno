Deno.test("foo 1 0", () => runDenoCommand("run --coverage foo.ts 1 0"));
Deno.test("foo 0 1", () => runDenoCommand("run --coverage foo.ts 0 1"));

async function runDenoCommand(args: string) {
  await new Deno.Command(Deno.execPath(), { args: args.split(" ") }).output();
}
