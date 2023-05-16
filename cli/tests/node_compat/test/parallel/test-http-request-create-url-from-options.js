// deno-fmt-ignore-file
// deno-lint-ignore-file

'use strict';
const common = require('../common');
const http = require('http');
const { urlToHttpOptions } = require('url');

const server = http.createServer((_, res) => {
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end();
});

server.listen(0, common.localhostIPv4, () => {
  const url = new URL(`http://${common.localhostIPv4}:${server.address().port}/anypath`);
  const httpOptions = urlToHttpOptions(url);

  delete httpOptions.href;

  const opts = {
      ...httpOptions,
      host: `${httpOptions.hostname}:${httpOptions.port}`,
  };

  const req = http.request(opts, common.mustCall(res => {
      res.on('data', () => { })
      res.on('end', common.mustCall(() => server.close()));
  }));

  req.on('error', common.mustNotCall());

  req.end();
});
