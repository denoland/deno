// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";
import { URLImpl } from "../web/url.ts";

export interface Start {
  args: string[];
  cwd: string;
  debugFlag: boolean;
  denoVersion: string;
  noColor: boolean;
  pid: number;
  repl: boolean;
  scriptUrl: URL;
  target: string;
  tsVersion: string;
  unstableFlag: boolean;
  v8Version: string;
  versionFlag: boolean;
}

export function opStart(): Start {
  const s = sendSync("op_start");
  s.scriptUrl = new URLImpl(s.scriptUrl);
  return s;
}

export interface Metrics {
  opsDispatched: number;
  opsDispatchedSync: number;
  opsDispatchedAsync: number;
  opsDispatchedAsyncUnref: number;
  opsCompleted: number;
  opsCompletedSync: number;
  opsCompletedAsync: number;
  opsCompletedAsyncUnref: number;
  bytesSentControl: number;
  bytesSentData: number;
  bytesReceived: number;
}

export function metrics(): Metrics {
  return sendSync("op_metrics");
}
