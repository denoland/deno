import { createRequire } from "node:module";
{
  const mod = await import("package");
  console.log(mod.kind);
  console.log(mod);
}
{
  const require = createRequire(import.meta.url);
  const mod = require("package");
  console.log(mod);
}
