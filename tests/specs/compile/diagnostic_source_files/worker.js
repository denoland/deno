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

postMessage([
  getDiagnosticSourceLine(new URL("./embedded.txt", import.meta.url).href),
  getDiagnosticSourceLine(new URL("./host.txt", import.meta.url).href),
]);
