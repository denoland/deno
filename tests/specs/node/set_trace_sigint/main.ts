import { setTraceSigInt } from "node:util";

// Child: enable the SIGINT trace, then send ourselves SIGINT while stuck in a
// synchronous loop. The handler should print the stack trace and let the
// process be terminated *by* the signal.
if (Deno.env.get("SIGINT_TRACE_CHILD") === "1") {
  setTraceSigInt(true);

  new Deno.Command("kill", {
    args: ["-INT", String(Deno.pid)],
  }).spawn();

  function innerWork() {
    let sum = 0;
    // Busy loop so V8 can service the interrupt.
    for (let i = 0; i < 100_000_000_000; i++) {
      sum += i;
    }
    return sum;
  }

  function doWork() {
    return innerWork();
  }

  doWork();
} else {
  // Parent: run the child and verify it was killed by SIGINT (not a clean
  // exit) after printing Node's `KEYBOARD_INTERRUPT` banner and a stack trace.
  const { code, signal, stderr } = await new Deno.Command(Deno.execPath(), {
    args: ["run", "--allow-all", import.meta.filename!],
    env: { SIGINT_TRACE_CHILD: "1" },
  }).output();

  const text = new TextDecoder().decode(stderr);
  console.log("signal:", signal);
  console.log("code:", code);
  console.log(
    text.includes("KEYBOARD_INTERRUPT") ? "banner: ok" : "banner: MISSING",
  );
  console.log(text.includes("innerWork") ? "trace: ok" : "trace: MISSING");
}
