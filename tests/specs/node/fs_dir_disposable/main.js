import fsPromises from "node:fs/promises";
import { opendirSync } from "node:fs";

// Test Symbol.asyncDispose via `await using`
{
  await using dir = await fsPromises.opendir(".");
  void dir;
}

// Test Symbol.dispose via `using`
{
  using dir = opendirSync(".");
  void dir;
}

console.log("ok");
