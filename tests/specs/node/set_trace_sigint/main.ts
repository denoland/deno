import { setTraceSigInt } from "node:util";

setTraceSigInt(true);

function innerWork() {
  let sum = 0;
  for (let i = 0; i < 100_000_000_000; i++) {
    sum += i;
  }
  return sum;
}

function doWork() {
  return innerWork();
}

// Spawn a helper that will send us SIGINT
new Deno.Command("kill", {
  args: ["-INT", String(Deno.pid)],
}).spawn();

doWork();
