import { listenAndServe } from "https://deno.land/std/http/server.ts";
console.log("http://localhost:4500/");
listenAndServe(":4500", (_req) => new Response("Hello World\n"));
