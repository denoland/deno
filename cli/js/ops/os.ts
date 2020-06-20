// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../core.ts";
import { errors } from "../errors.ts";

export function loadavg(): number[] {
  return core.dispatchJson.sendSync("op_loadavg");
}

export function hostname(): string {
  return core.dispatchJson.sendSync("op_hostname");
}

export function osRelease(): string {
  return core.dispatchJson.sendSync("op_os_release");
}

export function exit(code = 0): never {
  core.dispatchJson.sendSync("op_exit", { code });
  throw new Error("Code not reachable");
}

function setEnv(key: string, value: string): void {
  core.dispatchJson.sendSync("op_set_env", { key, value });
}

function getEnv(key: string): string | undefined {
  return core.dispatchJson.sendSync("op_get_env", { key })[0];
}

function deleteEnv(key: string): void {
  core.dispatchJson.sendSync("op_delete_env", { key });
}

export const env = {
  get: getEnv,
  toObject(): { [key: string]: string } {
    return core.dispatchJson.sendSync("op_env");
  },
  set: setEnv,
  delete: deleteEnv,
};

type DirKind =
  | "home"
  | "cache"
  | "config"
  | "executable"
  | "data"
  | "data_local"
  | "audio"
  | "desktop"
  | "document"
  | "download"
  | "font"
  | "picture"
  | "public"
  | "template"
  | "tmp"
  | "video";

export function dir(kind: DirKind): string | null {
  try {
    return core.dispatchJson.sendSync("op_get_dir", { kind });
  } catch (error) {
    if (error instanceof errors.PermissionDenied) {
      throw error;
    }
    return null;
  }
}

export function execPath(): string {
  return core.dispatchJson.sendSync("op_exec_path");
}
