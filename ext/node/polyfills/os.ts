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

// deno-lint-ignore-file prefer-primordials no-process-global

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  op_cpus,
  op_homedir,
  op_node_os_get_priority,
  op_node_os_set_priority,
  op_node_os_user_info,
} = core.ops;

const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");
const { os } = core.loadExtScript(
  "ext:deno_node/internal_binding/constants.ts",
);
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { osUptime } = core.loadExtScript("ext:deno_os/30_os.js");
const { validateInt32 } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { denoErrorToNodeSystemError } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

const {
  ObjectDefineProperties,
  StringPrototypeEndsWith,
  StringPrototypeSlice,
} = primordials;

const constants = os;

function arch() {
  return process.arch;
}

availableParallelism[Symbol.toPrimitive] = () => availableParallelism();
arch[Symbol.toPrimitive] = () => process.arch;
endianness[Symbol.toPrimitive] = () => endianness();
freemem[Symbol.toPrimitive] = () => freemem();
homedir[Symbol.toPrimitive] = () => homedir();
hostname[Symbol.toPrimitive] = () => hostname();
platform[Symbol.toPrimitive] = () => platform();
release[Symbol.toPrimitive] = () => release();
version[Symbol.toPrimitive] = () => version();
totalmem[Symbol.toPrimitive] = () => totalmem();
type[Symbol.toPrimitive] = () => type();
uptime[Symbol.toPrimitive] = () => uptime();
machine[Symbol.toPrimitive] = () => machine();
tmpdir[Symbol.toPrimitive] = () => tmpdir();

function cpus() {
  return op_cpus();
}

function endianness() {
  const buffer = new ArrayBuffer(2);
  new DataView(buffer).setInt16(0, 256, true /* littleEndian */);
  return new Int16Array(buffer)[0] === 256 ? "LE" : "BE";
}

function freemem() {
  if (Deno.build.os === "linux" || Deno.build.os == "android") {
    return Deno.systemMemoryInfo().available;
  } else {
    return Deno.systemMemoryInfo().free;
  }
}

function getPriority(pid = 0) {
  validateInt32(pid, "pid");
  try {
    return op_node_os_get_priority(pid);
  } catch (error) {
    throw denoErrorToNodeSystemError(error, "uv_os_getpriority");
  }
}

function homedir() {
  return op_homedir();
}

function hostname() {
  return Deno.hostname();
}

function loadavg() {
  if (isWindows) {
    return [0, 0, 0];
  }
  return Deno.loadavg();
}

function networkInterfaces() {
  const interfaces = {};
  for (
    const { name, address, netmask, family, mac, scopeid, cidr } of Deno
      .networkInterfaces()
  ) {
    const addresses = interfaces[name] ||= [];
    const networkAddress = {
      address,
      netmask,
      family,
      mac,
      internal: (family === "IPv4" && isIPv4LoopbackAddr(address)) ||
        (family === "IPv6" && isIPv6LoopbackAddr(address)),
      cidr,
    };
    if (family === "IPv6") {
      networkAddress.scopeid = scopeid;
    }
    addresses.push(networkAddress);
  }
  return interfaces;
}

function isIPv4LoopbackAddr(addr) {
  return addr.startsWith("127");
}

function isIPv6LoopbackAddr(addr) {
  return addr === "::1" || addr === "fe80::1";
}

function platform() {
  return process.platform;
}

function release() {
  return Deno.osRelease();
}

function version() {
  return Deno.osRelease();
}

function machine() {
  if (Deno.build.arch == "aarch64") {
    return "arm64";
  }

  return Deno.build.arch;
}

function setPriority(pid, priority) {
  if (priority === undefined) {
    priority = pid;
    pid = 0;
  }

  validateInt32(pid, "pid");
  validateInt32(priority, "priority", -20, 19);

  try {
    op_node_os_set_priority(pid, priority);
  } catch (error) {
    throw denoErrorToNodeSystemError(error, "uv_os_setpriority");
  }
}

function tmpdir() {
  if (isWindows) {
    let temp = Deno.env.get("TEMP") || Deno.env.get("TMP") ||
      (Deno.env.get("SystemRoot") || Deno.env.get("windir")) + "\\temp";
    if (
      temp.length > 1 && StringPrototypeEndsWith(temp, "\\") &&
      !StringPrototypeEndsWith(temp, ":\\")
    ) {
      temp = StringPrototypeSlice(temp, 0, -1);
    }

    return temp;
  } else {
    let temp = Deno.env.get("TMPDIR") || Deno.env.get("TMP") ||
      Deno.env.get("TEMP") || "/tmp";
    if (temp.length > 1 && StringPrototypeEndsWith(temp, "/")) {
      temp = StringPrototypeSlice(temp, 0, -1);
    }
    return temp;
  }
}

function totalmem() {
  return Deno.systemMemoryInfo().total;
}

function type() {
  switch (Deno.build.os) {
    case "windows":
      return "Windows_NT";
    case "linux":
    case "android":
      return "Linux";
    case "darwin":
      return "Darwin";
    case "freebsd":
      return "FreeBSD";
    case "openbsd":
      return "OpenBSD";
    default:
      throw new Error("unreachable");
  }
}

function uptime() {
  return osUptime();
}

function userInfo(
  options = { encoding: "utf-8" },
) {
  let uid = Deno.uid();
  let gid = Deno.gid();

  if (isWindows) {
    uid = -1;
    gid = -1;
  }
  let { username, homedir: hd, shell } = op_node_os_user_info(uid);

  if (options?.encoding === "buffer") {
    hd = hd ? Buffer.from(hd) : hd;
    shell = shell ? Buffer.from(shell) : shell;
    username = Buffer.from(username);
  }

  return {
    uid,
    gid,
    homedir: hd,
    shell,
    username,
  };
}

function availableParallelism() {
  return navigator.hardwareConcurrency;
}

const EOL = isWindows ? "\r\n" : "\n";
const devNull = isWindows ? "\\\\.\\nul" : "/dev/null";

const mod = {
  availableParallelism,
  arch,
  cpus,
  endianness,
  freemem,
  getPriority,
  homedir,
  hostname,
  loadavg,
  networkInterfaces,
  machine,
  platform,
  release,
  setPriority,
  tmpdir,
  totalmem,
  type,
  uptime,
  userInfo,
  version,
};

ObjectDefineProperties(mod, {
  constants: {
    __proto__: null,
    configurable: false,
    enumerable: true,
    value: constants,
  },
  EOL: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    writable: false,
    value: EOL,
  },
  devNull: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    writable: false,
    value: devNull,
  },
});

return {
  "module.exports": mod,
  constants,
  arch,
  cpus,
  endianness,
  freemem,
  getPriority,
  homedir,
  hostname,
  loadavg,
  networkInterfaces,
  machine,
  platform,
  release,
  setPriority,
  tmpdir,
  totalmem,
  type,
  uptime,
  userInfo,
  version,
  availableParallelism,
  EOL,
  devNull,
  default: mod,
};
})();
