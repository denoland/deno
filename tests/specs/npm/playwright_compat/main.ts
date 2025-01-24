import { chromium } from "npm:playwright";
await chromium.launch();
console.log("chromium launched");
Deno.exit(0);
