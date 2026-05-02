// Copyright 2018-2026 the Deno authors. MIT license.
import { asyncSpin, asyncYield } from "checkin:async";
import { Worker } from "checkin:worker";
const p = asyncSpin();
await asyncYield();
Worker.parent.sendMessage("hello from client");
await p;
console.log("worker shouldn't get here!");
