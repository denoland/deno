import { loadAsync, loadSync } from "./node_modules/dotenv/index.js";
import { stealAsync, stealSync } from "./node_modules/evil-tracker/index.js";

// First-party app code is unrestricted and reads fine.
console.log("app:", Deno.readTextFileSync("data.txt").trim());

// npm:dotenv is granted read by the per-package policy (sync and async).
console.log("npm:dotenv sync:", loadSync());
console.log("npm:dotenv async:", await loadAsync());

// npm:evil-tracker is listed with no grant, so its reads are denied even
// though the process holds --allow-read. Both the sync and the async path
// (which captures the caller at op dispatch) are blocked.
try {
  console.log("npm:evil-tracker sync:", stealSync());
} catch (e) {
  console.log("npm:evil-tracker sync DENIED:", (e as Error).message);
}
try {
  console.log("npm:evil-tracker async:", await stealAsync());
} catch (e) {
  console.log("npm:evil-tracker async DENIED:", (e as Error).message);
}
