import { parseModelKeys } from "./rce_only.js";

// With the mitigation enabled the dangerous "constructor" key is stripped even
// though this module contains no thenable pattern for the DoS (stage 2) patch.
console.log("keys:", JSON.stringify(parseModelKeys("a:constructor:b")));
