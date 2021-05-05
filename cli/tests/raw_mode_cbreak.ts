Deno.setRaw(0, true);
Deno.setRaw(0, true, { cbreak: true }); // Can be called multiple times

const signal = Deno.signals.interrupt();

Deno.stdout.writeSync(new TextEncoder().encode("S"));

await signal;

Deno.stdout.writeSync(new TextEncoder().encode("A"));

signal.dispose();

Deno.setRaw(0, false); // restores old mode.
Deno.setRaw(0, false); // Can be safely called multiple times
