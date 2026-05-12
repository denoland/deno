// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

(function () {
const { core } = globalThis.__bootstrap;
const { promises } = core.loadExtScript("ext:deno_node/dns.ts");

return {
  default: promises,
  getDefaultResultOrder: promises.getDefaultResultOrder,
  getServers: promises.getServers,
  lookup: promises.lookup,
  lookupService: promises.lookupService,
  resolve: promises.resolve,
  resolve4: promises.resolve4,
  resolve6: promises.resolve6,
  resolveAny: promises.resolveAny,
  resolveCaa: promises.resolveCaa,
  resolveCname: promises.resolveCname,
  resolveMx: promises.resolveMx,
  resolveNaptr: promises.resolveNaptr,
  resolveNs: promises.resolveNs,
  resolvePtr: promises.resolvePtr,
  Resolver: promises.Resolver,
  resolveSoa: promises.resolveSoa,
  resolveSrv: promises.resolveSrv,
  resolveTxt: promises.resolveTxt,
  reverse: promises.reverse,
  setDefaultResultOrder: promises.setDefaultResultOrder,
  setServers: promises.setServers,
  NODATA: promises.NODATA,
  FORMERR: promises.FORMERR,
  SERVFAIL: promises.SERVFAIL,
  NOTFOUND: promises.NOTFOUND,
  NOTIMP: promises.NOTIMP,
  REFUSED: promises.REFUSED,
  BADQUERY: promises.BADQUERY,
  BADNAME: promises.BADNAME,
  BADFAMILY: promises.BADFAMILY,
  BADRESP: promises.BADRESP,
  CONNREFUSED: promises.CONNREFUSED,
  TIMEOUT: promises.TIMEOUT,
  EOF: promises.EOF,
  FILE: promises.FILE,
  NOMEM: promises.NOMEM,
  DESTRUCTION: promises.DESTRUCTION,
  BADSTR: promises.BADSTR,
  BADFLAGS: promises.BADFLAGS,
  NONAME: promises.NONAME,
  BADHINTS: promises.BADHINTS,
  NOTINITIALIZED: promises.NOTINITIALIZED,
  LOADIPHLPAPI: promises.LOADIPHLPAPI,
  ADDRGETNETWORKPARAMS: promises.ADDRGETNETWORKPARAMS,
  CANCELLED: promises.CANCELLED,
};
})();
