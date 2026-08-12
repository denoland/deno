// Proof that v8::TerminateExecution interrupts a synchronous hot loop —
// the second test must still run after the first is killed.
Deno.test({
  name: "spins forever",
  timeout: 100,
  fn() {
    while (true) {}
  },
});

Deno.test("runs after the spin", () => {});
