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

// This module ports:
// - https://github.com/nodejs/node/blob/master/src/cares_wrap.cc
// - https://github.com/nodejs/node/blob/master/src/cares_wrap.h

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayPrototypeFilter,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeReverse,
  ArrayPrototypeSort,
  Error,
  MathMin,
  MathPow,
  Number,
  NumberParseInt,
  NumberPrototypeToString,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  SafeArrayIterator,
  SafeRegExp,
  SafeSet,
  SafeSetIterator,
  SetPrototypeAdd,
  SetPrototypeClear,
  SetPrototypeDelete,
  SetPrototypeHas,
  StringPrototypeIncludes,
  StringPrototypePadStart,
  StringPrototypeReplace,
  StringPrototypeSplit,
  Symbol,
} = primordials;
const {
  op_dns_resolve,
  op_net_get_ips_from_perm_token,
  op_net_get_system_dns_servers,
  op_node_getaddrinfo,
  op_node_getnameinfo,
} = core.ops;
const { isIPv4, isIPv6 } = core.loadExtScript(
  "ext:deno_node/internal/net.ts",
);
const { codeMap } = core.loadExtScript(
  "ext:deno_node/internal_binding/uv.ts",
);
const {
  AsyncWrap,
  providerType,
} = core.loadExtScript("ext:deno_node/internal_binding/async_wrap.ts");
const { ares_strerror } = core.loadExtScript(
  "ext:deno_node/internal_binding/ares.ts",
);
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");

interface LookupAddress {
  address: string;
  family: number;
}

interface ErrnoException extends Error {
  errno?: number;
  code?: string;
  path?: string;
  syscall?: string;
}

const DNS_ORDER_VERBATIM = 0;
const DNS_ORDER_IPV4_FIRST = 1;
const DNS_ORDER_IPV6_FIRST = 2;

// Module-private marker placed on the getaddrinfo completion callback used by
// net.connect's *built-in* lookup. Only a callback bearing this symbol is
// handed the NetPermToken from a lookup, so the token never escapes to a
// user-supplied dns.lookup callback or a custom net.connect `lookup` function.
// User code cannot reference this symbol, so it can neither receive the token
// nor forge the marker. See GHSA-fhjh-jqv7-m238.
const kPermTokenSink = Symbol("kPermTokenSink");

class GetAddrInfoReqWrap extends AsyncWrap {
  family!: number;
  hostname!: string;
  port: number | undefined;

  callback!: (
    err: ErrnoException | null,
    addressOrAddresses?: string | LookupAddress[] | null,
    family?: number,
  ) => void;
  resolve!: (addressOrAddresses: LookupAddress | LookupAddress[]) => void;
  reject!: (err: ErrnoException | null) => void;
  oncomplete!: (
    err: number | null,
    addresses: string[],
    netPermToken: object | undefined,
  ) => void;

  constructor() {
    super(providerType.GETADDRINFOREQWRAP);
  }
}

function getaddrinfo(
  req: GetAddrInfoReqWrap,
  hostname: string,
  family: number,
  _hints: number,
  order: 0 | 1 | 2,
): number {
  let addresses: string[] = [];

  // TODO(cmorten): use hints
  // REF: https://nodejs.org/api/dns.html#dns_supported_getaddrinfo_flags

  (async () => {
    let error = 0;
    let netPermToken: object | undefined;
    try {
      netPermToken = await op_node_getaddrinfo(
        hostname,
        req.port || undefined,
        family,
      );
      ArrayPrototypePush(
        addresses,
        ...new SafeArrayIterator(
          op_net_get_ips_from_perm_token(netPermToken),
        ),
      );
      if (addresses.length === 0) {
        error = codeMap.get("EAI_NODATA")!;
      }
    } catch (e) {
      if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotCapable.prototype, e)) {
        error = codeMap.get("EPERM")!;
      } else if (
        typeof (e as { uv_errcode?: number })?.uv_errcode === "number" &&
        (e as { uv_errcode: number }).uv_errcode !== 0
      ) {
        // Propagate the real libuv error code reported by `getaddrinfo`
        // (e.g. `EAI_NONAME`/`ENOTFOUND`) instead of flattening every failure
        // to `EAI_NODATA`, so the resulting error matches Node.js.
        error = (e as { uv_errcode: number }).uv_errcode;
      } else {
        error = codeMap.get("EAI_NODATA")!;
      }
    }

    // REF: https://github.com/nodejs/node/blob/0e157b6cd8694424ea9d8a1c1854fd1d08cbb064/src/cares_wrap.cc#L1739
    if (order === DNS_ORDER_IPV4_FIRST) {
      ArrayPrototypeSort(addresses, (a: string, b: string): number => {
        if (isIPv4(a)) {
          return -1;
        } else if (isIPv4(b)) {
          return 1;
        }

        return 0;
      });
    } else if (order === DNS_ORDER_IPV6_FIRST) {
      ArrayPrototypeSort(addresses, (a: string, b: string): number => {
        if (isIPv6(a)) {
          return -1;
        } else if (isIPv6(b)) {
          return 1;
        }
        return 0;
      });
    }

    if (family === 4) {
      addresses = ArrayPrototypeFilter(addresses, (addr) => isIPv4(addr));
    } else if (family === 6) {
      addresses = ArrayPrototypeFilter(addresses, (addr) => isIPv6(addr));
    }

    req.oncomplete(error, addresses, netPermToken);
  })();

  return 0;
}

class GetNameInfoReqWrap extends AsyncWrap {
  address!: string;
  port!: number;

  callback?: (
    err: ErrnoException | null,
    hostname?: string,
    service?: string,
  ) => void;
  resolve!: (result: { hostname: string; service: string }) => void;
  reject!: (err: ErrnoException | null) => void;
  oncomplete!: (
    err: Error | null,
    hostname?: string,
    service?: string,
  ) => void;

  constructor() {
    super(providerType.GETNAMEINFOREQWRAP);
  }
}

function getnameinfo(
  req: GetNameInfoReqWrap,
  address: string,
  port: number,
): number {
  (async () => {
    try {
      const result = await op_node_getnameinfo(address, port);
      req.oncomplete(null, result[0], result[1]);
    } catch (err) {
      req.oncomplete(err as Error);
    }
  })();
  return 0;
}

class QueryReqWrap extends AsyncWrap {
  bindingName!: string;
  hostname!: string;
  ttl!: boolean;

  callback!: (
    err: ErrnoException | null,
    // deno-lint-ignore no-explicit-any
    records?: any,
  ) => void;
  // deno-lint-ignore no-explicit-any
  resolve!: (records: any) => void;
  reject!: (err: ErrnoException | null) => void;
  oncomplete!: (
    err: number,
    // deno-lint-ignore no-explicit-any
    records: any,
    ttls?: number[],
  ) => void;

  constructor() {
    super(providerType.QUERYWRAP);
  }
}

interface ChannelWrapQuery {
  queryAny(req: QueryReqWrap, name: string): number;
  queryA(req: QueryReqWrap, name: string): number;
  queryAaaa(req: QueryReqWrap, name: string): number;
  queryCaa(req: QueryReqWrap, name: string): number;
  queryCname(req: QueryReqWrap, name: string): number;
  queryMx(req: QueryReqWrap, name: string): number;
  queryNs(req: QueryReqWrap, name: string): number;
  queryTxt(req: QueryReqWrap, name: string): number;
  querySrv(req: QueryReqWrap, name: string): number;
  queryPtr(req: QueryReqWrap, name: string): number;
  queryNaptr(req: QueryReqWrap, name: string): number;
  querySoa(req: QueryReqWrap, name: string): number;
  getHostByAddr(req: QueryReqWrap, name: string): number;
}

const trailingDotRegExp = new SafeRegExp(/\.$/);

function fqdnToHostname(fqdn: string): string {
  return StringPrototypeReplace(fqdn, trailingDotRegExp, "");
}

let systemDnsServers: [string, number][] | null = null;

function getSystemDnsServers(): [string, number][] {
  if (systemDnsServers !== null) {
    return systemDnsServers;
  }

  systemDnsServers = op_net_get_system_dns_servers();
  return systemDnsServers;
}

class ChannelWrap extends AsyncWrap implements ChannelWrapQuery {
  #servers: [string, number][] | null = null;
  #timeout: number;
  #tries: number;
  #maxTimeout: number;
  #pendingQueries: Set<QueryReqWrap> = new SafeSet();
  #cancelRids: Set<number> = new SafeSet();

  constructor(timeout: number, tries: number, maxTimeout: number) {
    super(providerType.DNSCHANNEL);

    this.#timeout = timeout;
    this.#tries = tries;
    this.#maxTimeout = maxTimeout;
  }

  async #query(
    query: string,
    recordType: Deno.RecordType,
    ttl?: boolean,
  ) {
    // deno-lint-ignore no-explicit-any
    let code: any;
    let ret: Awaited<ReturnType<typeof Deno.resolveDns>>;

    if (this.#servers !== null && this.#servers.length) {
      for (const server of new SafeArrayIterator(this.#servers)) {
        const ipAddr = server[0];
        const port = server[1];
        const resolveOptions = {
          nameServer: {
            ipAddr,
            port,
          },
        };

        ({ code, ret } = await this.#resolve(
          query,
          recordType,
          resolveOptions,
          ttl,
        ));

        if (
          code === 0 || code === codeMap.get("EAI_NODATA")! ||
          code === "ETIMEOUT"
        ) {
          break;
        }
      }
    } else {
      ({ code, ret } = await this.#resolve(query, recordType, null, ttl));
    }

    return { code: code!, ret: ret! };
  }

  async #resolve(
    query: string,
    recordType: Deno.RecordType,
    resolveOptions?: Deno.ResolveDnsOptions,
    ttl?: boolean,
  ): Promise<{
    // deno-lint-ignore no-explicit-any
    code: any;
    // deno-lint-ignore no-explicit-any
    ret: any[];
  }> {
    const tries = this.#tries > 0 ? this.#tries : 1;

    for (let attempt = 0; attempt < tries; attempt++) {
      let ret = [];
      // deno-lint-ignore no-explicit-any
      let code: any = 0;

      // Always create a cancel handle so cancel() can abort in-flight ops.
      const cancelRid = core.createCancelHandle();
      this.#cancelRids.add(cancelRid);
      let timer: ReturnType<typeof setTimeout> | undefined;
      // Whether this attempt was aborted by its own timeout timer (rather than
      // by an explicit cancel()). The op is not given a timeout, so the manual
      // timer below is what enforces the per-attempt `timeout`; when it fires
      // the op throws `Interrupted`, which must be treated as a timeout and
      // retried, exactly like the `TimedOut` hickory raises when it wins first.
      let timedOut = false;

      try {
        if (this.#timeout >= 0) {
          // c-ares doubles timeout on each retry, capped by maxTimeout
          let currentTimeout = this.#timeout * MathPow(2, attempt);
          if (this.#maxTimeout >= 0) {
            currentTimeout = MathMin(currentTimeout, this.#maxTimeout);
          }
          timer = setTimeout(() => {
            timedOut = true;
            this.#cancelRids.delete(cancelRid);
            core.tryClose(cancelRid);
          }, currentTimeout);
        }

        const res = await op_dns_resolve({
          query,
          recordType,
          options: resolveOptions,
          cancelRid,
        }, /* useEdns0 */ false);
        if (ttl) {
          ret = res;
        } else {
          ret = ArrayPrototypeMap(res, (recordWithTtl) => recordWithTtl.data);
        }
        return { code, ret };
      } catch (e) {
        if (
          ObjectPrototypeIsPrototypeOf(Deno.errors.Interrupted.prototype, e)
        ) {
          if (timedOut) {
            // Our own timeout timer aborted the op - this is a timeout, so
            // retry with the next (longer) timeout if attempts remain.
            if (attempt < tries - 1) continue;
          }
          // Either the attempts are exhausted or this was an explicit cancel();
          // in both cases stop and report a timeout.
          code = "ETIMEOUT";
        } else if (
          ObjectPrototypeIsPrototypeOf(Deno.errors.TimedOut.prototype, e)
        ) {
          // TimedOut from hickory - retry if attempts remain
          if (attempt < tries - 1) continue;
          code = "ETIMEOUT";
        } else if (
          ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, e)
        ) {
          code = codeMap.get("EAI_NODATA")!;
        } else {
          // TODO(cmorten): map errors to appropriate error codes.
          code = codeMap.get("UNKNOWN")!;
        }
        return { code, ret };
      } finally {
        if (timer !== undefined) clearTimeout(timer);
        this.#cancelRids.delete(cancelRid);
        core.tryClose(cancelRid);
      }
    }

    return { code: codeMap.get("UNKNOWN")!, ret: [] };
  }

  queryAny(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(
      // deno-lint-ignore no-explicit-any
      this.#query(name, "ANY" as any, true),
      ({ code, ret }) => {
        if (!SetPrototypeHas(this.#pendingQueries, req)) return;
        SetPrototypeDelete(this.#pendingQueries, req);

        if (code !== 0) {
          req.oncomplete(code, []);
          return;
        }

        const records: { type: string; [key: string]: unknown }[] = [];
        for (const entry of new SafeArrayIterator(ret)) {
          const data = entry?.data ?? entry;
          const ttl = entry?.ttl ?? 0;
          const rt = entry?.recordType;

          switch (rt) {
            case "A":
              ArrayPrototypePush(records, { type: "A", address: data, ttl });
              break;
            case "AAAA":
              ArrayPrototypePush(records, {
                type: "AAAA",
                address: data,
                ttl,
              });
              break;
            case "MX":
              ArrayPrototypePush(records, {
                type: "MX",
                priority: data.preference,
                exchange: fqdnToHostname(data.exchange),
              });
              break;
            case "NS":
              ArrayPrototypePush(records, {
                type: "NS",
                value: fqdnToHostname(data),
              });
              break;
            case "TXT":
              ArrayPrototypePush(records, { type: "TXT", entries: data });
              break;
            case "PTR":
              ArrayPrototypePush(records, {
                type: "PTR",
                value: fqdnToHostname(data),
              });
              break;
            case "SOA":
              ArrayPrototypePush(records, {
                type: "SOA",
                nsname: fqdnToHostname(data.mname),
                hostmaster: fqdnToHostname(data.rname),
                serial: data.serial,
                refresh: data.refresh,
                retry: data.retry,
                expire: data.expire,
                minttl: data.minimum,
              });
              break;
            case "CAA":
              ArrayPrototypePush(records, {
                type: "CAA",
                [data.tag]: data.value,
                critical: +data.critical && 128,
              });
              break;
            case "CNAME":
              ArrayPrototypePush(records, { type: "CNAME", value: data });
              break;
            case "NAPTR":
              ArrayPrototypePush(records, {
                type: "NAPTR",
                order: data.order,
                preference: data.preference,
                flags: data.flags,
                service: data.services,
                regexp: data.regexp,
                replacement: data.replacement,
              });
              break;
            case "SRV":
              ArrayPrototypePush(records, {
                type: "SRV",
                priority: data.priority,
                weight: data.weight,
                port: data.port,
                name: fqdnToHostname(data.target),
              });
              break;
          }
        }

        const err = records.length ? 0 : codeMap.get("EAI_NODATA")!;
        req.oncomplete(err, records);
      },
    );

    return 0;
  }

  queryA(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "A", req.ttl), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      let recordsWithTtl;
      if (req.ttl) {
        recordsWithTtl = ArrayPrototypeMap(
          ret as Deno.RecordWithTtl[],
          (val) => ({
            address: val?.data,
            ttl: val?.ttl,
          }),
        );
      }

      req.oncomplete(code, recordsWithTtl ?? ret);
    });

    return 0;
  }

  queryAaaa(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(
      this.#query(name, "AAAA", req.ttl),
      ({ code, ret }) => {
        if (!SetPrototypeHas(this.#pendingQueries, req)) return;
        SetPrototypeDelete(this.#pendingQueries, req);

        let recordsWithTtl;
        if (req.ttl) {
          recordsWithTtl = ArrayPrototypeMap(
            ret as Deno.RecordWithTtl[],
            (val) => ({
              address: val?.data as string,
              ttl: val?.ttl,
            }),
          );
        }

        req.oncomplete(code, recordsWithTtl ?? ret);
      },
    );

    return 0;
  }

  queryCaa(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "CAA"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as Deno.CaaRecord[],
        ({ critical, tag, value }) => ({
          [tag]: value,
          critical: +critical && 128,
        }),
      );

      req.oncomplete(code, records);
    });

    return 0;
  }

  queryCname(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "CNAME"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      req.oncomplete(code, ret);
    });

    return 0;
  }

  queryMx(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "MX"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as Deno.MxRecord[],
        ({ preference, exchange }) => ({
          priority: preference,
          exchange: fqdnToHostname(exchange),
        }),
      );

      req.oncomplete(code, records);
    });

    return 0;
  }

  queryNaptr(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "NAPTR"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as Deno.NaptrRecord[],
        ({ order, preference, flags, services, regexp, replacement }) => ({
          flags,
          service: services,
          regexp,
          replacement,
          order,
          preference,
        }),
      );

      req.oncomplete(code, records);
    });

    return 0;
  }

  queryNs(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "NS"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as string[],
        (record) => fqdnToHostname(record),
      );

      req.oncomplete(code, records);
    });

    return 0;
  }

  queryPtr(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "PTR"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as string[],
        (record) => fqdnToHostname(record),
      );

      req.oncomplete(code, records);
    });

    return 0;
  }

  querySoa(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "SOA"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      let record = {};

      if (ret.length) {
        const { mname, rname, serial, refresh, retry, expire, minimum } =
          ret[0] as Deno.SoaRecord;

        record = {
          nsname: fqdnToHostname(mname),
          hostmaster: fqdnToHostname(rname),
          serial,
          refresh,
          retry,
          expire,
          minttl: minimum,
        };
      }

      req.oncomplete(code, record);
    });

    return 0;
  }

  querySrv(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "SRV"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as Deno.SrvRecord[],
        ({ priority, weight, port, target }) => ({
          priority,
          weight,
          port,
          name: fqdnToHostname(target),
        }),
      );

      req.oncomplete(code, records);
    });

    return 0;
  }

  queryTxt(req: QueryReqWrap, name: string): number {
    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(name, "TXT"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      req.oncomplete(code, ret);
    });

    return 0;
  }

  getHostByAddr(req: QueryReqWrap, name: string): number {
    let reverseName: string;

    if (isIPv4(name)) {
      const octets = StringPrototypeSplit(name, ".");
      reverseName = ArrayPrototypeJoin(ArrayPrototypeReverse(octets), ".") +
        ".in-addr.arpa";
    } else if (isIPv6(name)) {
      // Expand the IPv6 address to full form
      const parts = StringPrototypeSplit(name, ":");
      const expanded: string[] = [];
      let emptyFound = false;
      for (const part of new SafeArrayIterator(parts)) {
        if (part === "" && !emptyFound) {
          emptyFound = true;
          const missing = 8 -
            ArrayPrototypeFilter(parts, (p) => p !== "").length;
          for (let j = 0; j < missing; j++) {
            ArrayPrototypePush(expanded, "0000");
          }
        } else if (part !== "" && StringPrototypeIncludes(part, ".")) {
          // IPv4-mapped IPv6 (e.g. ::ffff:1.2.3.4) - convert dotted
          // quad to two 16-bit hex groups
          const octets = ArrayPrototypeMap(
            StringPrototypeSplit(part, "."),
            Number,
          );
          ArrayPrototypePush(
            expanded,
            StringPrototypePadStart(
              NumberPrototypeToString((octets[0] << 8) | octets[1], 16),
              4,
              "0",
            ),
          );
          ArrayPrototypePush(
            expanded,
            StringPrototypePadStart(
              NumberPrototypeToString((octets[2] << 8) | octets[3], 16),
              4,
              "0",
            ),
          );
        } else if (part !== "") {
          ArrayPrototypePush(expanded, StringPrototypePadStart(part, 4, "0"));
        }
      }
      const fullHex = ArrayPrototypeJoin(expanded, "");
      reverseName = ArrayPrototypeJoin(
        ArrayPrototypeReverse(StringPrototypeSplit(fullHex, "")),
        ".",
      ) + ".ip6.arpa";
    } else {
      req.oncomplete(codeMap.get("EINVAL")!, []);
      return 0;
    }

    SetPrototypeAdd(this.#pendingQueries, req);

    PromisePrototypeThen(this.#query(reverseName, "PTR"), ({ code, ret }) => {
      if (!SetPrototypeHas(this.#pendingQueries, req)) return;
      SetPrototypeDelete(this.#pendingQueries, req);

      const records = ArrayPrototypeMap(
        ret as string[],
        (record) => fqdnToHostname(record),
      );
      req.oncomplete(code, records);
    });

    return 0;
  }

  getServers(): [string, number][] {
    if (this.#servers === null) {
      return getSystemDnsServers();
    }
    return this.#servers;
  }

  setServers(servers: string | [number, string, number][]): number {
    if (typeof servers === "string") {
      const tuples: [string, number][] = [];

      for (let i = 0; i < servers.length; i += 2) {
        ArrayPrototypePush(tuples, [
          servers[i],
          NumberParseInt(servers[i + 1]),
        ]);
      }

      this.#servers = tuples;
    } else {
      this.#servers = ArrayPrototypeMap(
        servers,
        (server) => [server[1], server[2]],
      );
    }

    return 0;
  }

  setLocalAddress(_addr0: string, _addr1?: string) {
    notImplemented("cares.ChannelWrap.prototype.setLocalAddress");
  }

  cancel() {
    for (const req of new SafeSetIterator(this.#pendingQueries)) {
      req.oncomplete("ECANCELLED", []);
    }
    SetPrototypeClear(this.#pendingQueries);

    // Abort in-flight DNS operations so the process can exit.
    for (const rid of new SafeSetIterator(this.#cancelRids)) {
      core.tryClose(rid);
    }
    SetPrototypeClear(this.#cancelRids);
  }
}

const DNS_ESETSRVPENDING = -1000;
const EMSG_ESETSRVPENDING = "There are pending queries.";

function strerror(code: number) {
  return code === DNS_ESETSRVPENDING
    ? EMSG_ESETSRVPENDING
    : ares_strerror(code);
}

return {
  DNS_ORDER_VERBATIM,
  DNS_ORDER_IPV4_FIRST,
  DNS_ORDER_IPV6_FIRST,
  GetAddrInfoReqWrap,
  getaddrinfo,
  GetNameInfoReqWrap,
  getnameinfo,
  QueryReqWrap,
  ChannelWrap,
  strerror,
  kPermTokenSink,
  default: {
    DNS_ORDER_VERBATIM,
    DNS_ORDER_IPV4_FIRST,
    DNS_ORDER_IPV6_FIRST,
    GetAddrInfoReqWrap,
    getaddrinfo,
    getnameinfo,
    QueryReqWrap,
    ChannelWrap,
    strerror,
  },
};
})();
