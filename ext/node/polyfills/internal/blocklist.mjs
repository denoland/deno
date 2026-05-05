// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  op_blocklist_add_address,
  op_blocklist_add_range,
  op_blocklist_add_subnet,
  op_blocklist_check,
  op_blocklist_new,
  op_socket_address_get_serialization,
  op_socket_address_parse,
} = core.ops;

const {
  validateInt32,
  validateObject,
  validatePort,
  validateString,
  validateUint32,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { customInspectSymbol } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const { inspect } = core.loadExtScript(
  "ext:deno_node/internal/util/inspect.mjs",
);

const {
  ArrayIsArray,
  ArrayPrototypeUnshift,
  JSONParse,
  NumberParseInt,
  RegExpPrototypeExec,
  SafeArrayIterator,
  SafeRegExp,
  StringPrototypeIncludes,
  StringPrototypeToLowerCase,
  Symbol,
} = primordials;

const kIPv4SubnetRe = new SafeRegExp(
  "Subnet: IPv4 (\\d{1,3}(?:\\.\\d{1,3}){3})\\/(\\d{1,2})",
);
const kIPv4AddressRe = new SafeRegExp(
  "Address: IPv4 (\\d{1,3}(?:\\.\\d{1,3}){3})",
);
const kIPv4RangeRe = new SafeRegExp(
  "Range: IPv4 (\\d{1,3}(?:\\.\\d{1,3}){3})-(\\d{1,3}(?:\\.\\d{1,3}){3})",
);
const kIPv6SubnetRe = new SafeRegExp(
  "Subnet: IPv6 ([0-9a-fA-F:]{1,39})\\/([0-9]{1,3})",
  "i",
);
const kIPv6AddressRe = new SafeRegExp(
  "Address: IPv6 ([0-9a-fA-F:]{1,39})",
  "i",
);
const kIPv6RangeRe = new SafeRegExp(
  "Range: IPv6 ([0-9a-fA-F:]{1,39})-([0-9a-fA-F:]{1,39})",
  "i",
);

const internalBlockList = Symbol("blocklist");
const kRules = Symbol("rules");

function formatFamily(family) {
  return family === "ipv6" ? "IPv6" : "IPv4";
}

class BlockList {
  constructor() {
    this[internalBlockList] = op_blocklist_new();
    this[kRules] = [];
  }

  static isBlockList(value) {
    return value?.[internalBlockList] !== undefined;
  }

  [customInspectSymbol](depth, options) {
    if (depth < 0) {
      return this;
    }

    const opts = {
      ...options,
      depth: options.depth == null ? null : options.depth - 1,
    };

    return `BlockList ${
      inspect({
        rules: this[kRules],
      }, opts)
    }`;
  }

  addAddress(address, family = "ipv4") {
    if (!SocketAddress.isSocketAddress(address)) {
      validateString(address, "address");
      validateString(family, "family");
      new SocketAddress({
        address,
        family,
      });
    } else {
      family = address.family;
      address = address.address;
    }
    op_blocklist_add_address(this[internalBlockList], address);
    ArrayPrototypeUnshift(
      this[kRules],
      `Address: ${formatFamily(StringPrototypeToLowerCase(family))} ${address}`,
    );
  }

  addRange(start, end, family = "ipv4") {
    if (!SocketAddress.isSocketAddress(start)) {
      validateString(start, "start");
      validateString(family, "family");
      new SocketAddress({
        address: start,
        family,
      });
    } else {
      family = start.family;
      start = start.address;
    }
    if (!SocketAddress.isSocketAddress(end)) {
      validateString(end, "end");
      validateString(family, "family");
      new SocketAddress({
        address: end,
        family,
      });
    } else {
      end = end.address;
    }
    const ret = op_blocklist_add_range(this[internalBlockList], start, end);
    if (ret === false) {
      throw new ERR_INVALID_ARG_VALUE("start", start, "must come before end");
    }
    ArrayPrototypeUnshift(
      this[kRules],
      `Range: ${
        formatFamily(StringPrototypeToLowerCase(family))
      } ${start}-${end}`,
    );
  }

  addSubnet(network, prefix, family = "ipv4") {
    if (!SocketAddress.isSocketAddress(network)) {
      validateString(network, "network");
      validateString(family, "family");
      new SocketAddress({
        address: network,
        family,
      });
    } else {
      family = network.family;
      network = network.address;
    }
    family = StringPrototypeToLowerCase(family);
    switch (family) {
      case "ipv4":
        validateInt32(prefix, "prefix", 0, 32);
        break;
      case "ipv6":
        validateInt32(prefix, "prefix", 0, 128);
        break;
    }
    op_blocklist_add_subnet(this[internalBlockList], network, prefix);
    ArrayPrototypeUnshift(
      this[kRules],
      `Subnet: ${formatFamily(family)} ${network}/${prefix}`,
    );
  }

  check(address, family = "ipv4") {
    if (!SocketAddress.isSocketAddress(address)) {
      validateString(address, "address");
      validateString(family, "family");
      try {
        new SocketAddress({
          address,
          family,
        });
      } catch {
        // Ignore the error. If it's not a valid address, return false.
        return false;
      }
    } else {
      family = address.family;
      address = address.address;
    }
    try {
      return op_blocklist_check(this[internalBlockList], address, family);
    } catch (_) {
      // Node API expects false as return value if the address is invalid.
      // Example: `blocklist.check("1.1.1.1", "ipv6")` should return false.
      return false;
    }
  }

  get rules() {
    return this[kRules];
  }

  toJSON() {
    return this[kRules];
  }

  fromJSON(data) {
    if (ArrayIsArray(data)) {
      for (const n of new SafeArrayIterator(data)) {
        if (typeof n !== "string") {
          throw new ERR_INVALID_ARG_TYPE("data", ["string", "string[]"], data);
        }
      }
    } else if (typeof data !== "string") {
      throw new ERR_INVALID_ARG_TYPE("data", ["string", "string[]"], data);
    } else {
      data = JSONParse(data);
      if (!ArrayIsArray(data)) {
        throw new ERR_INVALID_ARG_TYPE("data", ["string", "string[]"], data);
      }
      for (const n of new SafeArrayIterator(data)) {
        if (typeof n !== "string") {
          throw new ERR_INVALID_ARG_TYPE("data", ["string", "string[]"], data);
        }
      }
    }
    parseIPInfo(this, data);
  }
}

function parseIPInfo(self, data) {
  for (const item of new SafeArrayIterator(data)) {
    if (StringPrototypeIncludes(item, "IPv4")) {
      const subnetMatch = RegExpPrototypeExec(kIPv4SubnetRe, item);
      if (subnetMatch) {
        self.addSubnet(subnetMatch[1], NumberParseInt(subnetMatch[2]));
        continue;
      }
      const addressMatch = RegExpPrototypeExec(kIPv4AddressRe, item);
      if (addressMatch) {
        self.addAddress(addressMatch[1]);
        continue;
      }
      const rangeMatch = RegExpPrototypeExec(kIPv4RangeRe, item);
      if (rangeMatch) {
        self.addRange(rangeMatch[1], rangeMatch[2]);
        continue;
      }
    }
    if (StringPrototypeIncludes(item, "IPv6")) {
      const ipv6SubnetMatch = RegExpPrototypeExec(kIPv6SubnetRe, item);
      if (ipv6SubnetMatch) {
        self.addSubnet(
          ipv6SubnetMatch[1],
          NumberParseInt(ipv6SubnetMatch[2]),
          "ipv6",
        );
        continue;
      }
      const ipv6AddressMatch = RegExpPrototypeExec(kIPv6AddressRe, item);
      if (ipv6AddressMatch) {
        self.addAddress(ipv6AddressMatch[1], "ipv6");
        continue;
      }
      const ipv6RangeMatch = RegExpPrototypeExec(kIPv6RangeRe, item);
      if (ipv6RangeMatch) {
        self.addRange(ipv6RangeMatch[1], ipv6RangeMatch[2], "ipv6");
        continue;
      }
    }
  }
}

const kDetail = Symbol("kDetail");

class SocketAddress {
  static isSocketAddress(value) {
    return value?.[kDetail] !== undefined;
  }

  constructor(options = kEmptyObject) {
    validateObject(options, "options");
    let { family = "ipv4" } = options;
    const {
      address = (family === "ipv4" ? "127.0.0.1" : "::"),
      port = 0,
      flowlabel = 0,
    } = options;

    if (typeof family?.toLowerCase === "function") {
      // deno-lint-ignore prefer-primordials
      family = family.toLowerCase();
    }
    switch (family) {
      case "ipv4":
        break;
      case "ipv6":
        break;
      default:
        throw new ERR_INVALID_ARG_VALUE("options.family", options.family);
    }

    validateString(address, "options.address");
    validatePort(port, "options.port");
    validateUint32(flowlabel, "options.flowlabel", false);

    this[kDetail] = {
      address,
      port,
      family,
      flowlabel,
    };
    const useInput = op_socket_address_parse(
      address,
      port,
      family,
    );
    if (!useInput) {
      const { 0: address_, 1: family_ } = op_socket_address_get_serialization();
      this[kDetail].address = address_;
      this[kDetail].family = family_;
    }
  }

  get address() {
    return this[kDetail].address;
  }

  get port() {
    return this[kDetail].port;
  }

  get family() {
    return this[kDetail].family;
  }

  get flowlabel() {
    // TODO(satyarohith): Implement this in Rust.
    // The flow label can be changed internally.
    return this[kDetail].flowlabel;
  }

  toJSON() {
    return {
      address: this.address,
      port: this.port,
      family: this.family,
      flowlabel: this.flowlabel,
    };
  }
}

return {
  BlockList,
  SocketAddress,
};
})();
