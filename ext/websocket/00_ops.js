// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { core } from "ext:core/mod.js";
const {
  op_ws_create,
  op_ws_close,
  op_ws_send_binary,
  op_ws_send_binary_ab,
  op_ws_send_text,
  op_ws_next_event,
  op_ws_get_buffer,
  op_ws_get_buffer_as_string,
  op_ws_get_error,
  op_ws_send_ping,
  op_ws_get_buffered_amount,
  op_ws_send_text_async,
  op_ws_send_binary_async,
  op_ws_check_permission_and_cancel_handle,
} = core.ensureFastOps();

export {
  op_ws_check_permission_and_cancel_handle,
  op_ws_close,
  op_ws_create,
  op_ws_get_buffer,
  op_ws_get_buffer_as_string,
  op_ws_get_buffered_amount,
  op_ws_get_error,
  op_ws_next_event,
  op_ws_send_binary,
  op_ws_send_binary_ab,
  op_ws_send_binary_async,
  op_ws_send_ping,
  op_ws_send_text,
  op_ws_send_text_async,
};
