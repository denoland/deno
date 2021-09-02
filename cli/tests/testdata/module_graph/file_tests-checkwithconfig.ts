import { ServerRequest } from "../../../../test_util/std/http/server_legacy.ts";

export default (req: ServerRequest) => {
  req.respond({ body: `Hello, from Deno v${Deno.version.deno}!` });
};
