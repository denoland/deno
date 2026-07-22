import { chromium } from "npm:playwright";
// Pass an explicit timeout so a browser that never completes its CDP handshake
// rejects instead of hanging the whole spec shard until the CI job timeout.
const browser = await chromium.launch({ timeout: 60000 });
console.log("chromium launched");
await browser.close();
Deno.exit(0);
