// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** https://url.spec.whatwg.org/#idna */

import { sendSync } from "./dispatch_json.ts";

export function domainToAscii(
  domain: string,
  { beStrict = false }: { beStrict?: boolean } = {}
): string {
  return sendSync("op_domain_to_ascii", { domain, beStrict });
}
