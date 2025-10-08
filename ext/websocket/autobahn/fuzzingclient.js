// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file

import { $ } from "https://deno.land/x/dax@0.31.0/mod.ts";

$.setPrintCommand(true);
const pwd = new URL(".", import.meta.url).pathname;

const AUTOBAHN_TESTSUITE_DOCKER =
  "crossbario/autobahn-testsuite:25.10.1@sha256:519915fb568b04c9383f70a1c405ae3ff44ab9e35835b085239c258b6fac3074";

const self = Deno.execPath();
$`${self} run -A --config ${pwd}/../../../tests/config/deno.json ${pwd}/autobahn_server.js`
  .spawn();

for (let i = 0; i < 6; i++) {
  try {
    await $`docker pull ${AUTOBAHN_TESTSUITE_DOCKER}`;
    break;
  } catch (e) {
    $.logError(`error: docker pull failed ${e}, waiting 10s`);
    await new Promise((r) => setTimeout(r, 10000));
  }
}

await $`docker run 
  --name fuzzingserver
  -v ${pwd}/fuzzingclient.json:/fuzzingclient.json:ro
  -v ${pwd}/reports:/reports
  -p 9001:9001
  --net=host
  --rm ${AUTOBAHN_TESTSUITE_DOCKER}
  wstest -m fuzzingclient -s fuzzingclient.json`
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
