import { runEcho as runEcho2 } from "http://localhost:4545/run/warn_on_deprecated_api/mod.ts";

const p = Deno.run({
  cmd: [
    Deno.execPath(),
    "eval",
    "console.log('hello world')",
  ],
});
await p.status();
p.close();

async function runEcho() {
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

await runEcho();
await runEcho();

for (let i = 0; i < 10; i++) {
  await runEcho();
}

await runEcho2();
