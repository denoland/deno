// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./window";
import * as workersGuest from "./workers_guest";

// This variable functioning correctly depends on `declareAsLet`
// in //tools/ts_library_builder/main.ts
window.onmessage = workersGuest.onmessage;

window.workerMain = workersGuest.workerMain;
window.workerClose = workersGuest.workerClose;
window.postMessage = workersGuest.postMessage;
