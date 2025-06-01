export function addAsync(a: number, b: number): Promise<number> {
  const worker = new Worker(import.meta.resolve("./worker.ts"), {
    type: "module",
  });

  return new Promise((resolve) => {
    worker.addEventListener("message", (event) => {
      resolve(event.data);
    });

    worker.postMessage({ a, b });
  });
}
