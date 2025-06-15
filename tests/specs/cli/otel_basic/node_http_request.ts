import http from "node:http";
import { text } from "node:stream/consumers";

function request(url: string) {
  return new Promise((resolve) => {
    http.request(url, (res) => resolve(text(res))).end();
  });
}

await request("http://localhost:4545/echo.ts");
await request("http://localhost:4545/not-found");
