// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch";
import { sendSync } from "./dispatch_json";

// TODO(bartlomieju): move to `repl.ts`?
export function formatError(errString: string): string {
  const res = sendSync(dispatch.OP_FORMAT_ERROR, { error: errString });
  return res.error;
}
