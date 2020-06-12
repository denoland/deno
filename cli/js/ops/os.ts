// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";
import { errors } from "../errors.ts";

export function loadavg(): number[] {
  return sendSync("op_loadavg");
}

export function hostname(): string {
  return sendSync("op_hostname");
}

export function osRelease(): string {
  return sendSync("op_os_release");
}

export function exit(code = 0): never {
  sendSync("op_exit", { code });
  throw new Error("Code not reachable");
}

function setEnv(key: string, value: string): void {
  sendSync("op_set_env", { key, value });
}

function getEnv(key: string): string | undefined {
  return sendSync("op_get_env", { key })[0];
}

function deleteEnv(key: string): void {
  sendSync("op_delete_env", { key });
}

export const env = {
  get: getEnv,
  toObject(): { [key: string]: string } {
    return sendSync("op_env");
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
    return sendSync("op_get_dir", { kind });
  } catch (error) {
    if (error instanceof errors.PermissionDenied) {
      throw error;
    }
    return null;
  }
}

export function execPath(): string {
  return sendSync("op_exec_path");
}
