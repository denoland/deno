// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ net: true }, async function httpServerBasic() {
  const addr = "127.0.0.1:4501";
  let counter = 0;
  const server = deno.httpListen(addr);
  const serverComplete = server.serve(
    (req: deno.ServerRequest, res: deno.ServerResponse) => {
      assertEqual(req.url, "/foo");
      assertEqual(req.method, "GET");
      assertEqual(req.headers, [["host", "127.0.0.1:4501"]]);

      res.status = 404;
      res.headers = [["content-type", "text/plain"], ["hello", "world"]];
      const resBody = new TextEncoder().encode("404 Not Found\n");
      res.writeResponse(resBody);
      counter++;
      server.close();
    }
  );

  const fetchRes = await fetch("http://" + addr + "/foo");
  // TODO
  // assertEqual(fetchRes.headers, [
  //   [ "content-type", "text/plain" ],
  //   [ "hello", "world" ],
  // ]);
  // assertEqual(fetchRes.statusText, "Not Found");
  assertEqual(fetchRes.status, 404);
  const body = await fetchRes.text();
  assertEqual(body, "404 Not Found\n");

  await serverComplete;
  assertEqual(counter, 1);
});
