// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// The following are all the process APIs that don't depend on the stream module
// They have to be split this way to prevent a circular dependency

import { core, primordials } from "ext:core/mod.js";
const {
  Error,
  ObjectGetOwnPropertyNames,
  String,
  ReflectOwnKeys,
  ArrayPrototypeIncludes,
  Object,
  Proxy,
  ObjectPrototype,
  ObjectPrototypeIsPrototypeOf,
  ReflectDefineProperty,
  ReflectHas,
  TypeErrorPrototype,
} = primordials;
const { build, createLazyLoader } = core;

import { nextTick as _nextTick } from "ext:deno_node/_next_tick.ts";
import { _exiting } from "ext:deno_node/_process/exiting.ts";
import * as fs from "ext:deno_fs/30_fs.js";
import { ERR_INVALID_OBJECT_DEFINE_PROPERTY } from "ext:deno_node/internal/errors.ts";

const loadProcess = createLazyLoader<NodeJS.Process>("node:process");
let nodeProcess: NodeJS.Process | undefined;

/** Returns the operating system CPU architecture for which the Deno binary was compiled */
export function arch(): string {
  if (build.arch == "x86_64") {
    return "x64";
  } else if (build.arch == "aarch64") {
    return "arm64";
  } else if (build.arch == "riscv64gc") {
    return "riscv64";
  } else {
    throw new Error("unreachable");
  }
}

/** https://nodejs.org/api/process.html#process_process_chdir_directory */
export const chdir = fs.chdir;

/** https://nodejs.org/api/process.html#process_process_cwd */
export const cwd = fs.cwd;

/** https://nodejs.org/api/process.html#process_process_nexttick_callback_args */
export const nextTick = _nextTick;

/** Wrapper of Deno.env.get, which doesn't throw type error when
 * the env name has "=" or "\0" in it. */
function denoEnvGet(name: string) {
  try {
    return Deno.env.get(name);
  } catch (e) {
    if (ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e)) {
      return undefined;
    }
    throw e;
  }
}

const OBJECT_PROTO_PROP_NAMES = ObjectGetOwnPropertyNames(ObjectPrototype);
/**
 * https://nodejs.org/api/process.html#process_process_env
 * Requires env permissions
 */
export const env:
  & InstanceType<ObjectConstructor>
  & Record<string | symbol, string> = new Proxy(Object(), {
    get: (target, prop) => {
      if (typeof prop === "symbol") {
        return target[prop];
      }

      const envValue = denoEnvGet(prop);

      if (envValue) {
        return envValue;
      }

      if (ArrayPrototypeIncludes(OBJECT_PROTO_PROP_NAMES, prop)) {
        return target[prop];
      }

      return envValue;
    },
    ownKeys: () => ReflectOwnKeys(Deno.env.toObject()),
    getOwnPropertyDescriptor: (_target, name) => {
      const value = denoEnvGet(String(name));
      if (value !== undefined) {
        return {
          enumerable: true,
          configurable: true,
          value,
        };
      }
    },
    set(target, prop, value) {
      if (typeof prop === "symbol") {
        target[prop] = value;
        return true;
      }

      if (typeof value !== "string") {
        nodeProcess ??= loadProcess();
        nodeProcess.emitWarning(
          "Assigning any value other than a string, number, or boolean to a " +
            "process.env property is deprecated. Please make sure to convert the value " +
            "to a string before setting process.env with it.",
          "DeprecationWarning",
          "DEP0104",
        );
      }

      Deno.env.set(String(prop), String(value));
      return true; // success
    },
    has: (target, prop) => {
      if (typeof prop === "symbol") {
        return ReflectHas(target, prop);
      }

      return typeof denoEnvGet(prop) === "string";
    },
    deleteProperty(target, key) {
      if (typeof key === "symbol") {
        delete target[key];
        return true;
      }

      Deno.env.delete(String(key));
      return true;
    },
    defineProperty(target, property, attributes) {
      if (attributes?.get || attributes?.set) {
        throw new ERR_INVALID_OBJECT_DEFINE_PROPERTY(
          "'process.env' does not accept an " +
            "accessor(getter/setter) descriptor",
        );
      }

      if (
        !attributes?.configurable || !attributes?.enumerable ||
        !attributes?.writable
      ) {
        throw new ERR_INVALID_OBJECT_DEFINE_PROPERTY(
          "'process.env' only accepts a " +
            "configurable, writable," +
            " and enumerable data descriptor",
        );
      }

      if (typeof property === "symbol") {
        ReflectDefineProperty(target, property, attributes);
        return true;
      }

      Deno.env.set(String(property), String(attributes?.value));
      return true;
    },
  });

/**
 * https://nodejs.org/api/process.html#process_process_version
 *
 * This value is hard coded to latest stable release of Node, as
 * some packages are checking it for compatibility. Previously
 * it pointed to Deno version, but that led to incompability
 * with some packages.
 */
export const version = "v24.2.0";

/**
 * https://nodejs.org/api/process.html#process_process_versions
 *
 * This value is hard coded to latest stable release of Node, as
 * some packages are checking it for compatibility. Previously
 * it contained only output of `Deno.version`, but that led to incompability
 * with some packages. Value of `v8` field is still taken from `Deno.version`.
 */
export const versions = {
  node: "24.2.0",
  uv: "1.43.0",
  zlib: "1.2.11",
  brotli: "1.0.9",
  ares: "1.18.1",
  modules: "108",
  nghttp2: "1.47.0",
  napi: "8",
  llhttp: "6.0.10",
  openssl: "3.0.7+quic",
  cldr: "41.0",
  icu: "71.1",
  tz: "2022b",
  unicode: "14.0",
  ngtcp2: "0.8.1",
  nghttp3: "0.7.0",
  sqlite: "3.49.0",
  // Will be filled when calling "__bootstrapNodeProcess()",
  deno: "",
  v8: "",
  typescript: "",
};
