import { ServerRequest } from "jsr:@std/http/server";

export default (req: ServerRequest) => {
  req.respond({ body: `Hello, from Deno v${Deno.version.deno}!` });
};
