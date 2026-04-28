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

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import type { ErrnoException } from "ext:deno_node/internal/errors.ts";
import { isIPv4, isIPv6 } from "ext:deno_node/internal/net.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import {
  AsyncWrap,
  providerType,
} from "ext:deno_node/internal_binding/async_wrap.ts";
import { ares_strerror } from "ext:deno_node/internal_binding/ares.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { core } from "ext:core/mod.js";
import {
  op_dns_resolve,
  op_net_get_ips_from_perm_token,
  op_net_get_system_dns_servers,
  op_node_getaddrinfo,
  op_node_getnameinfo,
} from "ext:core/ops";

interface LookupAddress {
  address: string;
  family: number;
}

export const DNS_ORDER_VERBATIM = 0;
export const DNS_ORDER_IPV4_FIRST = 1;
export const DNS_ORDER_IPV6_FIRST = 2;

export class GetAddrInfoReqWrap extends AsyncWrap {
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

export function getaddrinfo(
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
      addresses.push(...op_net_get_ips_from_perm_token(netPermToken));
      if (addresses.length === 0) {
        error = codeMap.get("EAI_NODATA")!;
      }
    } catch (e) {
      if (e instanceof Deno.errors.NotCapable) {
        error = codeMap.get("EPERM")!;
      } else {
        error = codeMap.get("EAI_NODATA")!;
      }
    }

    // REF: https://github.com/nodejs/node/blob/0e157b6cd8694424ea9d8a1c1854fd1d08cbb064/src/cares_wrap.cc#L1739
    if (order === DNS_ORDER_IPV4_FIRST) {
      addresses.sort((a: string, b: string): number => {
        if (isIPv4(a)) {
          return -1;
        } else if (isIPv4(b)) {
          return 1;
        }

        return 0;
      });
    } else if (order === DNS_ORDER_IPV6_FIRST) {
      addresses.sort((a: string, b: string): number => {
        if (isIPv6(a)) {
          return -1;
        } else if (isIPv6(b)) {
          return 1;
        }
        return 0;
      });
    }

    if (family === 4) {
      addresses = addresses.filter((addr) => isIPv4(addr));
    } else if (family === 6) {
      addresses = addresses.filter((addr) => isIPv6(addr));
    }

    req.oncomplete(error, addresses, netPermToken);
  })();

  return 0;
}

export class GetNameInfoReqWrap extends AsyncWrap {
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

export function getnameinfo(
  req: GetNameInfoReqWrap,
  address: string,
  port: number,
): number {
  (async () => {
    try {
      const [hostname, service] = await op_node_getnameinfo(address, port);
      req.oncomplete(null, hostname, service);
    } catch (err) {
      req.oncomplete(err as Error);
    }
  })();
  return 0;
}

export class QueryReqWrap extends AsyncWrap {
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

export interface ChannelWrapQuery {
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

function fqdnToHostname(fqdn: string): string {
  return fqdn.replace(/\.$/, "");
}

let systemDnsServers: [string, number][] | null = null;

function getSystemDnsServers(): [string, number][] {
  if (systemDnsServers !== null) {
    return systemDnsServers;
  }

  systemDnsServers = op_net_get_system_dns_servers();
  return systemDnsServers;
}

export class ChannelWrap extends AsyncWrap implements ChannelWrapQuery {
  #servers: [string, number][] | null = null;
  #timeout: number;
  #tries: number;
  #maxTimeout: number;
  #pendingQueries: Set<QueryReqWrap> = new Set();
  #cancelRids: Set<number> = new Set();

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
      for (const [ipAddr, port] of this.#servers) {
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

      try {
        if (this.#timeout >= 0) {
          // c-ares doubles timeout on each retry, capped by maxTimeout
          let currentTimeout = this.#timeout * Math.pow(2, attempt);
          if (this.#maxTimeout >= 0) {
            currentTimeout = Math.min(currentTimeout, this.#maxTimeout);
          }
          timer = setTimeout(() => {
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
          ret = res.map((recordWithTtl) => recordWithTtl.data);
        }
        return { code, ret };
      } catch (e) {
        if (
          e instanceof Deno.errors.Interrupted ||
          e instanceof Deno.errors.TimedOut
        ) {
          if (attempt < tries - 1) continue;
          code = "ETIMEOUT";
        } else if (e instanceof Deno.errors.NotFound) {
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
    this.#pendingQueries.add(req);

    // deno-lint-ignore no-explicit-any
    this.#query(name, "ANY" as any, true).then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      if (code !== 0) {
        req.oncomplete(code, []);
        return;
      }

      const records: { type: string; [key: string]: unknown }[] = [];
      for (const entry of ret) {
        const data = entry?.data ?? entry;
        const ttl = entry?.ttl ?? 0;
        const rt = entry?.recordType;

        switch (rt) {
          case "A":
            records.push({ type: "A", address: data, ttl });
            break;
          case "AAAA":
            records.push({ type: "AAAA", address: data, ttl });
            break;
          case "MX":
            records.push({
              type: "MX",
              priority: data.preference,
              exchange: fqdnToHostname(data.exchange),
            });
            break;
          case "NS":
            records.push({ type: "NS", value: fqdnToHostname(data) });
            break;
          case "TXT":
            records.push({ type: "TXT", entries: data });
            break;
          case "PTR":
            records.push({ type: "PTR", value: fqdnToHostname(data) });
            break;
          case "SOA":
            records.push({
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
            records.push({
              type: "CAA",
              [data.tag]: data.value,
              critical: +data.critical && 128,
            });
            break;
          case "CNAME":
            records.push({ type: "CNAME", value: data });
            break;
          case "NAPTR":
            records.push({
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
            records.push({
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
    });

    return 0;
  }

  queryA(req: QueryReqWrap, name: string): number {
    this.#pendingQueries.add(req);

    this.#query(name, "A", req.ttl).then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      let recordsWithTtl;
      if (req.ttl) {
        recordsWithTtl = (ret as Deno.RecordWithTtl[]).map((val) => ({
          address: val?.data,
          ttl: val?.ttl,
        }));
      }

      req.oncomplete(code, recordsWithTtl ?? ret);
    });

    return 0;
  }

  queryAaaa(req: QueryReqWrap, name: string): number {
    this.#pendingQueries.add(req);

    this.#query(name, "AAAA", req.ttl).then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      let recordsWithTtl;
      if (req.ttl) {
        recordsWithTtl = (ret as Deno.RecordWithTtl[]).map((val) => ({
          address: val?.data as string,
          ttl: val?.ttl,
        }));
      }

      req.oncomplete(code, recordsWithTtl ?? ret);
    });

    return 0;
  }

  queryCaa(req: QueryReqWrap, name: string): number {
    this.#pendingQueries.add(req);

    this.#query(name, "CAA").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as Deno.CaaRecord[]).map(
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
    this.#pendingQueries.add(req);

    this.#query(name, "CNAME").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      req.oncomplete(code, ret);
    });

    return 0;
  }

  queryMx(req: QueryReqWrap, name: string): number {
    this.#pendingQueries.add(req);

    this.#query(name, "MX").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as Deno.MxRecord[]).map(
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
    this.#pendingQueries.add(req);

    this.#query(name, "NAPTR").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as Deno.NaptrRecord[]).map(
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
    this.#pendingQueries.add(req);

    this.#query(name, "NS").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as string[]).map((record) => fqdnToHostname(record));

      req.oncomplete(code, records);
    });

    return 0;
  }

  queryPtr(req: QueryReqWrap, name: string): number {
    this.#pendingQueries.add(req);

    this.#query(name, "PTR").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as string[]).map((record) => fqdnToHostname(record));

      req.oncomplete(code, records);
    });

    return 0;
  }

  querySoa(req: QueryReqWrap, name: string): number {
    this.#pendingQueries.add(req);

    this.#query(name, "SOA").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

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
    this.#pendingQueries.add(req);

    this.#query(name, "SRV").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as Deno.SrvRecord[]).map(
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
    this.#pendingQueries.add(req);

    this.#query(name, "TXT").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      req.oncomplete(code, ret);
    });

    return 0;
  }

  getHostByAddr(req: QueryReqWrap, name: string): number {
    let reverseName: string;

    if (isIPv4(name)) {
      const octets = name.split(".");
      reverseName = octets.reverse().join(".") + ".in-addr.arpa";
    } else if (isIPv6(name)) {
      // Expand the IPv6 address to full form
      const parts = name.split(":");
      const expanded: string[] = [];
      let emptyFound = false;
      for (const part of parts) {
        if (part === "" && !emptyFound) {
          emptyFound = true;
          const missing = 8 - parts.filter((p) => p !== "").length;
          for (let j = 0; j < missing; j++) {
            expanded.push("0000");
          }
        } else if (part !== "" && part.includes(".")) {
          // IPv4-mapped IPv6 (e.g. ::ffff:1.2.3.4) - convert dotted
          // quad to two 16-bit hex groups
          const octets = part.split(".").map(Number);
          expanded.push(
            ((octets[0] << 8) | octets[1]).toString(16).padStart(4, "0"),
          );
          expanded.push(
            ((octets[2] << 8) | octets[3]).toString(16).padStart(4, "0"),
          );
        } else if (part !== "") {
          expanded.push(part.padStart(4, "0"));
        }
      }
      const fullHex = expanded.join("");
      reverseName = fullHex.split("").reverse().join(".") + ".ip6.arpa";
    } else {
      req.oncomplete(codeMap.get("EINVAL")!, []);
      return 0;
    }

    this.#pendingQueries.add(req);

    this.#query(reverseName, "PTR").then(({ code, ret }) => {
      if (!this.#pendingQueries.has(req)) return;
      this.#pendingQueries.delete(req);

      const records = (ret as string[]).map((record) => fqdnToHostname(record));
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
        tuples.push([servers[i], parseInt(servers[i + 1])]);
      }

      this.#servers = tuples;
    } else {
      this.#servers = servers.map(([_ipVersion, ip, port]) => [ip, port]);
    }

    return 0;
  }

  setLocalAddress(_addr0: string, _addr1?: string) {
    notImplemented("cares.ChannelWrap.prototype.setLocalAddress");
  }

  cancel() {
    for (const req of this.#pendingQueries) {
      req.oncomplete("ECANCELLED", []);
    }
    this.#pendingQueries.clear();

    // Abort in-flight DNS operations so the process can exit.
    for (const rid of this.#cancelRids) {
      core.tryClose(rid);
    }
    this.#cancelRids.clear();
  }
}

const DNS_ESETSRVPENDING = -1000;
const EMSG_ESETSRVPENDING = "There are pending queries.";

export function strerror(code: number) {
  return code === DNS_ESETSRVPENDING
    ? EMSG_ESETSRVPENDING
    : ares_strerror(code);
}

export default {
  DNS_ORDER_VERBATIM,
  DNS_ORDER_IPV4_FIRST,
  DNS_ORDER_IPV6_FIRST,
  GetAddrInfoReqWrap,
  getaddrinfo,
  getnameinfo,
  QueryReqWrap,
  ChannelWrap,
  strerror,
};
