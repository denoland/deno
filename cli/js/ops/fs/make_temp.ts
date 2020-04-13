// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface MakeTempOptions {
  dir?: string;
  prefix?: string;
  suffix?: string;
}

export function makeTempDirSync(options: MakeTempOptions = {}): string {
  return sendSync("op_make_temp_dir", options);
}

export function makeTempDir(options: MakeTempOptions = {}): Promise<string> {
  return sendAsync("op_make_temp_dir", options);
}

export function makeTempFileSync(options: MakeTempOptions = {}): string {
  return sendSync("op_make_temp_file", options);
}

export function makeTempFile(options: MakeTempOptions = {}): Promise<string> {
  return sendAsync("op_make_temp_file", options);
}
