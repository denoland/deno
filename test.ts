import { run } from "deno";

import "./buffer_test.ts";
import "./bufio_test.ts";
import "./textproto_test.ts";
import { runTests, completePromise } from "./file_server_test.ts";

// file server test
const fileServer = run({
  args: ["deno", "--allow-net", "file_server.ts", "."]
});
// I am also too lazy to do this properly LOL
runTests(new Promise(res => setTimeout(res, 1000)));
(async () => {
  await completePromise;
  fileServer.close();
})();

// TODO import "./http_test.ts";
