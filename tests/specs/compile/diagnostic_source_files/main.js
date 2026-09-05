import { runInThisContext } from "node:vm";

function getDiagnosticSourceLine(fileName) {
  try {
    runInThisContext('throw new Error("diagnostic");', {
      filename: fileName,
    });
  } catch (error) {
    return Deno[Deno.internal].core.destructureError(error).sourceLine ??
      "NO_SOURCE_LINE";
  }
}

console.log(
  getDiagnosticSourceLine(new URL("./embedded.txt", import.meta.url).href),
);
console.log(
  getDiagnosticSourceLine(new URL("./host.txt", import.meta.url).href),
);

const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});
const workerLines = await new Promise((resolve, reject) => {
  worker.onmessage = (event) => resolve(event.data);
  worker.onerror = (event) => reject(event.error);
});
worker.terminate();
for (const line of workerLines) {
  console.log(line);
}
