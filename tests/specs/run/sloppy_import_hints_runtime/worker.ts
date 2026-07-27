const worker = new Worker(new URL("./worker_target.js", import.meta.url), {
  type: "module",
});

const message = await new Promise<string>((resolve) => {
  worker.addEventListener("error", (event) => {
    event.preventDefault();
    resolve(event.message);
  });
});
worker.terminate();

if (!message.includes("--sloppy-imports")) {
  throw new Error("Missing sloppy import worker suggestion");
}

console.log("worker suggestion");
