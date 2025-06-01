/// <reference no-default-lib="true" />
/// <reference lib="deno.worker" />

if (import.meta.main) {
  console.log("Hello from worker!");

  addEventListener("message", (evt) => {
    console.log(`Received ${evt.data}`);
    console.log("Closing");
    self.close();
  });
} else {
  console.log("worker.js imported from main thread");
}
