export async function runEcho() {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "console.log('hello world')",
    ],
  });
  await p.status();
  p.close();
}
