// PROTOTYPE — wayfinder P1.
import { whoamiSync } from "probe-esm";
self.postMessage(`worker-app=${globalThis.__permcapWhoami} pkg=${whoamiSync()}`);
