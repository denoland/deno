// Copyright 2018-2026 the Deno authors. MIT license.
import { Worker } from "checkin:worker";
Worker.parent.sendMessage("hello from client");
const message = await Worker.parent.receiveMessage();
console.log(`worker got from main "${message}"`);
