console.log(Deno.env.get("NODE_DEBUG") ?? "ok");
Deno.env.get("NOT_NODE_DEBUG");
