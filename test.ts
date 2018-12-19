#!/usr/bin/env deno --allow-run --allow-net
import { run } from "deno";

// colors tests
import "colors/main_test.ts";

// flags tests
import "flags/test.ts";

// net tests
import "net/bufio_test.ts";
import "net/http_test.ts";
import "net/textproto_test.ts";
import { runTests, completePromise } from "net/file_server_test.ts";

// logging tests
import "logging/test.ts";

// file server test
const fileServer = run({
  args: ["deno", "--allow-net", "net/file_server.ts", "."]
});
// path test
import "path/basename_test.ts";
import "path/dirname_test.ts";
import "path/extname_test.ts";
import "path/isabsolute_test.ts";
import "path/join_test.ts";
import "path/parse_format_test.ts";
import "path/relative_test.ts";
import "path/resolve_test.ts";
import "path/zero_length_strings_test.ts";

// I am also too lazy to do this properly LOL
runTests(new Promise(res => setTimeout(res, 5000)));
(async () => {
  await completePromise;
  fileServer.close();
})();
