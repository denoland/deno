// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

import { $ } from "https://deno.land/x/dax@0.31.0/mod.ts";

const pwd = new URL(".", import.meta.url).pathname;

const AUTOBAHN_TESTSUITE_DOCKER =
  "crossbario/autobahn-testsuite:0.8.2@sha256:5d4ba3aa7d6ab2fdbf6606f3f4ecbe4b66f205ce1cbc176d6cdf650157e52242";

const self = Deno.execPath();
$`${self} run -A --unstable ${pwd}/autobahn_server.js`.spawn();
await $`docker run --name fuzzingserver	-v ${pwd}/fuzzingclient.json:/fuzzingclient.json:ro	-v ${pwd}/reports:/reports -p 9001:9001	--net=host --rm	${AUTOBAHN_TESTSUITE_DOCKER} wstest -m fuzzingclient -s fuzzingclient.json`
  .cwd(pwd);

const { deno_websocket } = JSON.parse(
  Deno.readTextFileSync(`${pwd}/reports/servers/index.json`),
);
const result = Object.values(deno_websocket);

function failed(name) {
  return name != "OK" && name != "INFORMATIONAL" && name != "NON-STRICT";
}

const failedtests = result.filter((outcome) => failed(outcome.behavior));

console.log(
  `%c${result.length - failedtests.length} / ${result.length} tests OK`,
  `color: ${failedtests.length == 0 ? "green" : "red"}`,
);

Deno.exit(failedtests.length == 0 ? 0 : 1);
