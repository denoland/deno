// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendSync } from "./dispatch_json.ts";

export function localStorageInit(origin: string): {} {
  return sendSync("op_local_storage_init", { origin });
}

export function localStorageClear(): void {
  return sendSync("op_local_storage_clear", {});
}

export function localStorageGetItem(key: string): string | null {
  return sendSync("op_local_storage_get_item", { key }).value;
}

export function localStorageGetLength(): number {
  return sendSync("op_local_storage_get_length", {}).length;
}

export function localStorageSetItem(
  key: string,
  value: string
): { error?: string } {
  return sendSync("op_local_storage_set_item", { key, value });
}

export function localStorageRemoveItem(key: string): void {
  return sendSync("op_local_storage_remove_item", { key });
}
