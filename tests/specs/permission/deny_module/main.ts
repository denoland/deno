import { readSelf } from "./helper.ts";

try {
  const result = readSelf();
  console.log("ok:", result);
} catch (err) {
  console.log((err as Error).name + ": " + (err as Error).message);
  throw err;
}
