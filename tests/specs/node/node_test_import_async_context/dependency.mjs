import { setImmediate } from "node:timers/promises";
import { storage } from "./storage.mjs";

const before = storage.getStore();
await setImmediate();
export const contexts = [before, storage.getStore()];
