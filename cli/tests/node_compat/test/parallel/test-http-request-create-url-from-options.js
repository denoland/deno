// deno-fmt-ignore-file
// deno-lint-ignore-file

'use strict';
const common = require('../common');
const assert = require('assert');
const http = require('http');
const { urlToHttpOptions } = require('url');

const testPath = '/foo?bar';

const server = http.createServer(common.mustCall((req, res) => {
  assert.strictEqual(req.url, testPath);
  res.writeHead(200, { 'Content-Type': 'text/plain' });
  res.write('hello\n');
  res.end();
}));

server.listen(0, common.localhostIPv4, common.mustCall(() => {
  const httpOptions = urlToHttpOptions(new URL(`http://${common.localhostIPv4}:${server.address().port}${testPath}`))

  delete httpOptions.href

  const opts = {
    ...httpOptions,
    host: `${httpOptions.hostname}:${options.port}`
  };

  http.request(opts, common.mustCall(() => {
    server.close();
  }));
}));
