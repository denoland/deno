#!/usr/bin/env deno --allow-run --allow-net
import { run } from "deno";

import "net/bufio_test.ts";
import "net/http_test.ts";
import "net/textproto_test.ts";
import { runTests, completePromise } from "net/file_server_test.ts";

// file server test
const fileServer = run({
  args: ["deno", "--allow-net", "net/file_server.ts", "."]
});
// I am also too lazy to do this properly LOL
runTests(new Promise(res => setTimeout(res, 5000)));
(async () => {
  await completePromise;
  fileServer.close();
})();
