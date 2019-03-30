// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./globals";
import "./workers_globals";

import * as os from "./os";

// builtin modules
import "./deno";

export default async function denoMain(name: string): Promise<void> {
  os.start(name);
}
