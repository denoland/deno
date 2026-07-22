import { signals } from "./signals.ts";

for (const signal of signals) {
  Deno.addSignalListener(signal, () => {
    console.log("Received", signal);
    if (signal === "SIGTERM") {
      Deno.exit(0);
    }
  });
}

setInterval(() => {
  // keep alive
}, 1000);

console.log("Ready");
