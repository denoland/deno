import type Koa from "npm:@types/koa";
import type { Express } from "npm:@types/express";

declare const app: Koa;
app.use((ctx) => {
});

declare const app2: Express;
app2.post("/", (req, res) => {
});
