// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

const OP_FORMAT_ERROR = new JsonOp("format_error");

// TODO(bartlomieju): move to `repl.ts`?
export function formatError(errString: string): string {
  const res = OP_FORMAT_ERROR.sendSync({ error: errString });
  return res.error;
}
