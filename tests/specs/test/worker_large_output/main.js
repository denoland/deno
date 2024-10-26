// Regression test for workers that post large amounts of output as a test is ending. This
// test should not deadlock, though the output is undefined.
Deno.test(async function workerOutput() {
  console.log("Booting worker");
  const code =
    "self.postMessage(0); console.log(`hello from worker\n`.repeat(60000));";
  const worker = new Worker(URL.createObjectURL(new Blob([code])), {
    type: "module",
  });
  await new Promise((r) =>
    worker.addEventListener("message", () => {
      r();
    })
  );
});
