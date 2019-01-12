#!/usr/bin/env deno --allow-run --allow-net --allow-write
import { run } from "deno";

import "colors/test.ts";
import "datetime/test.ts";
import "examples/test.ts";
import "flags/test.ts";
import "logging/test.ts";
import "media_types/test.ts";
import "net/bufio_test.ts";
import "net/http_test.ts";
import "net/textproto_test.ts";
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
import "testing/test.ts";

import { runTests, completePromise } from "net/file_server_test.ts";

const fileServer = run({
  args: ["deno", "--allow-net", "net/file_server.ts", ".", "--cors"]
});

runTests(new Promise(res => setTimeout(res, 5000)));
(async () => {
  await completePromise;
  fileServer.close();
})();
