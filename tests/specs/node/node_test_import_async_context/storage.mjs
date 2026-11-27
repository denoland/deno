import { AsyncLocalStorage } from "node:async_hooks";

export const storage = new AsyncLocalStorage();
