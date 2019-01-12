#!/usr/bin/env deno --allow-run --allow-net --allow-write
import { run } from "deno";

import "colors/test.ts";
import "datetime/test.ts";
import "examples/test.ts";
import "flags/test.ts";
import "fs/mkdirp_test.ts";
import "fs/path/basename_test.ts";
import "fs/path/dirname_test.ts";
import "fs/path/extname_test.ts";
import "fs/path/isabsolute_test.ts";
import "fs/path/join_test.ts";
import "fs/path/parse_format_test.ts";
import "fs/path/relative_test.ts";
import "fs/path/resolve_test.ts";
import "fs/path/zero_length_strings_test.ts";
import "io/bufio_test.ts";
import "http/http_test.ts";
import "log/test.ts";
import "media_types/test.ts";
import "testing/test.ts";
import "textproto/test.ts";
import "ws/test.ts";

import { runTests, completePromise } from "http/file_server_test.ts";

const fileServer = run({
  args: ["deno", "--allow-net", "http/file_server.ts", ".", "--cors"]
});

runTests(new Promise(res => setTimeout(res, 5000)));
(async () => {
  await completePromise;
  fileServer.close();
})();
