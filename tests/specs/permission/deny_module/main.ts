import { readHostname } from "./helper.ts";

try {
  const text = readHostname();
  console.log("ok:", typeof text);
} catch (err) {
  console.log((err as Error).name + ": " + (err as Error).message);
  throw err;
}
