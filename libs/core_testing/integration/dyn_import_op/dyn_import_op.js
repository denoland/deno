// Copyright 2018-2025 the Deno authors. MIT license.
import "./main.js";
import { barrierAwait } from "checkin:async";
await barrierAwait("barrier");
console.log("done");
