// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
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

import { notImplemented } from "ext:deno_node/_utils.ts";
import { validateIntegerRange } from "ext:deno_node/_utils.ts";
import process from "ext:deno_node/process.ts";
import { isWindows, osType } from "ext:deno_node/_util/os.ts";
import { os } from "ext:deno_node/internal_binding/constants.ts";
import { osUptime } from "ext:runtime/30_os.js";
export const constants = os;

const SEE_GITHUB_ISSUE = "See https://github.com/denoland/deno_std/issues/1436";

interface CPUTimes {
  /** The number of milliseconds the CPU has spent in user mode */
  user: number;

  /** The number of milliseconds the CPU has spent in nice mode */
  nice: number;

  /** The number of milliseconds the CPU has spent in sys mode */
  sys: number;

  /** The number of milliseconds the CPU has spent in idle mode */
  idle: number;

  /** The number of milliseconds the CPU has spent in irq mode */
  irq: number;
}

interface CPUCoreInfo {
  model: string;

  /** in MHz */
  speed: number;

  times: CPUTimes;
}

interface NetworkAddress {
  /** The assigned IPv4 or IPv6 address */
  address: string;

  /** The IPv4 or IPv6 network mask */
  netmask: string;

  family: "IPv4" | "IPv6";

  /** The MAC address of the network interface */
  mac: string;

  /** true if the network interface is a loopback or similar interface that is not remotely accessible; otherwise false */
  internal: boolean;

  /** The numeric IPv6 scope ID (only specified when family is IPv6) */
  scopeid?: number;

  /** The assigned IPv4 or IPv6 address with the routing prefix in CIDR notation. If the netmask is invalid, this property is set to null. */
  cidr: string;
}

interface NetworkInterfaces {
  [key: string]: NetworkAddress[];
}

export interface UserInfoOptions {
  encoding: string;
}

interface UserInfo {
  username: string;
  uid: number;
  gid: number;
  shell: string;
  homedir: string;
}

export function arch(): string {
  return process.arch;
}

// deno-lint-ignore no-explicit-any
(arch as any)[Symbol.toPrimitive] = (): string => process.arch;
// deno-lint-ignore no-explicit-any
(endianness as any)[Symbol.toPrimitive] = (): string => endianness();
// deno-lint-ignore no-explicit-any
(freemem as any)[Symbol.toPrimitive] = (): number => freemem();
// deno-lint-ignore no-explicit-any
(homedir as any)[Symbol.toPrimitive] = (): string | null => homedir();
// deno-lint-ignore no-explicit-any
(hostname as any)[Symbol.toPrimitive] = (): string | null => hostname();
// deno-lint-ignore no-explicit-any
(platform as any)[Symbol.toPrimitive] = (): string => platform();
// deno-lint-ignore no-explicit-any
(release as any)[Symbol.toPrimitive] = (): string => release();
// deno-lint-ignore no-explicit-any
(version as any)[Symbol.toPrimitive] = (): string => version();
// deno-lint-ignore no-explicit-any
(totalmem as any)[Symbol.toPrimitive] = (): number => totalmem();
// deno-lint-ignore no-explicit-any
(type as any)[Symbol.toPrimitive] = (): string => type();
// deno-lint-ignore no-explicit-any
(uptime as any)[Symbol.toPrimitive] = (): number => uptime();

export function cpus(): CPUCoreInfo[] {
  return Array.from(Array(navigator.hardwareConcurrency)).map(() => {
    return {
      model: "",
      speed: 0,
      times: {
        user: 0,
        nice: 0,
        sys: 0,
        idle: 0,
        irq: 0,
      },
    };
  });
}

/**
 * Returns a string identifying the endianness of the CPU for which the Deno
 * binary was compiled. Possible values are 'BE' for big endian and 'LE' for
 * little endian.
 */
export function endianness(): "BE" | "LE" {
  // Source: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/DataView#Endianness
  const buffer = new ArrayBuffer(2);
  new DataView(buffer).setInt16(0, 256, true /* littleEndian */);
  // Int16Array uses the platform's endianness.
  return new Int16Array(buffer)[0] === 256 ? "LE" : "BE";
}

/** Return free memory amount */
export function freemem(): number {
  return Deno.systemMemoryInfo().free;
}

/** Not yet implemented */
export function getPriority(pid = 0): number {
  validateIntegerRange(pid, "pid");
  notImplemented(SEE_GITHUB_ISSUE);
}

/** Returns the string path of the current user's home directory. */
export function homedir(): string | null {
  // Note: Node/libuv calls getpwuid() / GetUserProfileDirectory() when the
  // environment variable isn't set but that's the (very uncommon) fallback
  // path. IMO, it's okay to punt on that for now.
  switch (osType) {
    case "windows":
      return Deno.env.get("USERPROFILE") || null;
    case "linux":
    case "darwin":
    case "freebsd":
      return Deno.env.get("HOME") || null;
    default:
      throw Error("unreachable");
  }
}

/** Returns the host name of the operating system as a string. */
export function hostname(): string {
  return Deno.hostname();
}

/** Returns an array containing the 1, 5, and 15 minute load averages */
export function loadavg(): number[] {
  if (isWindows) {
    return [0, 0, 0];
  }
  return Deno.loadavg();
}

/** Returns an object containing network interfaces that have been assigned a network address.
 * Each key on the returned object identifies a network interface. The associated value is an array of objects that each describe an assigned network address. */
export function networkInterfaces(): NetworkInterfaces {
  const interfaces: NetworkInterfaces = {};
  for (
    const { name, address, netmask, family, mac, scopeid, cidr } of Deno
      .networkInterfaces()
  ) {
    const addresses = interfaces[name] ||= [];
    const networkAddress: NetworkAddress = {
      address,
      netmask,
      family,
      mac,
      internal: (family === "IPv4" && isIPv4LoopbackAddr(address)) ||
        (family === "IPv6" && isIPv6LoopbackAddr(address)),
      cidr,
    };
    if (family === "IPv6") {
      networkAddress.scopeid = scopeid!;
    }
    addresses.push(networkAddress);
  }
  return interfaces;
}

function isIPv4LoopbackAddr(addr: string) {
  return addr.startsWith("127");
}

function isIPv6LoopbackAddr(addr: string) {
  return addr === "::1" || addr === "fe80::1";
}

/** Returns the a string identifying the operating system platform. The value is set at compile time. Possible values are 'darwin', 'linux', and 'win32'. */
export function platform(): string {
  return process.platform;
}

/** Returns the operating system as a string */
export function release(): string {
  return Deno.osRelease();
}

/** Returns a string identifying the kernel version */
export function version(): string {
  // TODO(kt3k): Temporarily uses Deno.osRelease().
  // Revisit this if this implementation is insufficient for any npm module
  return Deno.osRelease();
}

/** Not yet implemented */
export function setPriority(pid: number, priority?: number) {
  /* The node API has the 'pid' as the first parameter and as optional.
       This makes for a problematic implementation in Typescript. */
  if (priority === undefined) {
    priority = pid;
    pid = 0;
  }
  validateIntegerRange(pid, "pid");
  validateIntegerRange(priority, "priority", -20, 19);

  notImplemented(SEE_GITHUB_ISSUE);
}

/** Returns the operating system's default directory for temporary files as a string. */
export function tmpdir(): string | null {
  /* This follows the node js implementation, but has a few
     differences:
     * On windows, if none of the environment variables are defined,
       we return null.
     * On unix we use a plain Deno.env.get, instead of safeGetenv,
       which special cases setuid binaries.
     * Node removes a single trailing / or \, we remove all.
  */
  if (isWindows) {
    const temp = Deno.env.get("TEMP") || Deno.env.get("TMP");
    if (temp) {
      return temp.replace(/(?<!:)[/\\]*$/, "");
    }
    const base = Deno.env.get("SYSTEMROOT") || Deno.env.get("WINDIR");
    if (base) {
      return base + "\\temp";
    }
    return null;
  } else { // !isWindows
    const temp = Deno.env.get("TMPDIR") || Deno.env.get("TMP") ||
      Deno.env.get("TEMP") || "/tmp";
    return temp.replace(/(?<!^)\/*$/, "");
  }
}

/** Return total physical memory amount */
export function totalmem(): number {
  return Deno.systemMemoryInfo().total;
}

/** Returns operating system type (i.e. 'Windows_NT', 'Linux', 'Darwin') */
export function type(): string {
  switch (Deno.build.os as string) {
    case "windows":
      return "Windows_NT";
    case "linux":
      return "Linux";
    case "darwin":
      return "Darwin";
    case "freebsd":
      return "FreeBSD";
    default:
      throw Error("unreachable");
  }
}

/** Returns the Operating System uptime in number of seconds. */
export function uptime(): number {
  return osUptime();
}

/** Not yet implemented */
export function userInfo(
  // deno-lint-ignore no-unused-vars
  options: UserInfoOptions = { encoding: "utf-8" },
): UserInfo {
  notImplemented(SEE_GITHUB_ISSUE);
}

export const EOL = isWindows ? "\r\n" : "\n";
export const devNull = isWindows ? "\\\\.\\nul" : "/dev/null";

export default {
  arch,
  cpus,
  endianness,
  freemem,
  getPriority,
  homedir,
  hostname,
  loadavg,
  networkInterfaces,
  platform,
  release,
  setPriority,
  tmpdir,
  totalmem,
  type,
  uptime,
  userInfo,
  version,
  constants,
  EOL,
  devNull,
};
