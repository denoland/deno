import { runEcho as runEcho2 } from "http://localhost:4545/run/warn_on_deprecated_api/mod.ts";

// @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
