// PROTOTYPE — wayfinder P1. Throwaway.
// E3: point the denial at a real dependency graph. Runs express (a real CJS
// tree) with a package on the deny list and reports what breaks and how.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

console.log("deny list:", Deno.env.get("PERMCAP_DENY") ?? "(none)");
try {
  const express = require("express");
  const app = express();
  app.get("/", (_req, res) => res.json({ ok: true }));
  const server = app.listen(0, async () => {
    const port = server.address().port;
    try {
      const r = await fetch(`http://localhost:${port}/`);
      console.log("express responded:", r.status, await r.text());
    } catch (e) {
      console.log("request failed:", e.constructor.name, e.message);
    }
    server.close();
  });
} catch (e) {
  console.log("express failed to load:", e.constructor.name, e.message);
  console.log(e.stack?.split("\n").slice(0, 6).join("\n"));
}
