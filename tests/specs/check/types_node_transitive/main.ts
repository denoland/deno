// `@types/node` is not declared as a dependency here, but it comes in
// transitively via @types/express. Exactly one copy of the node types must be
// in the program, or the node and web globals below produce
// duplicate-identifier errors.
import type { Express } from "npm:@types/express";

declare const app: Express;
app.post("/", (_req, _res) => {});

process.cwd();
const _res: Response = new Response("ok");
