// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export interface MakeTempOptions {
  dir?: string;
  prefix?: string;
  suffix?: string;
}

export function makeTempDirSync(options: MakeTempOptions = {}): string {
  return core.dispatchJson.sendSync("op_make_temp_dir", options);
}

export function makeTempDir(options: MakeTempOptions = {}): Promise<string> {
  return core.dispatchJson.sendAsync("op_make_temp_dir", options);
}

export function makeTempFileSync(options: MakeTempOptions = {}): string {
  return core.dispatchJson.sendSync("op_make_temp_file", options);
}

export function makeTempFile(options: MakeTempOptions = {}): Promise<string> {
  return core.dispatchJson.sendAsync("op_make_temp_file", options);
}
