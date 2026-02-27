// Copyright 2018-2025 the Deno authors. MIT license.
import {
  op_async_throw_error_deferred,
  op_async_throw_error_eager,
  op_async_throw_error_lazy,
  op_error_custom_sync,
  op_error_custom_with_code_sync,
} from "ext:core/ops";

export async function asyncThrow(kind: "lazy" | "eager" | "deferred") {
  const op = {
    lazy: op_async_throw_error_lazy,
    eager: op_async_throw_error_eager,
    deferred: op_async_throw_error_deferred,
  }[kind];
  return await op();
}

export function throwCustomError(message: string) {
  op_error_custom_sync(message);
}

export function throwCustomErrorWithCode(message: string, code: number) {
  op_error_custom_with_code_sync(message, code);
}
