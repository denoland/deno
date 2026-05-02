async function request(url: string, options: any) {
  try {
    await (await fetch(url, options)).text();
  } catch {
  }
}

await request("http://localhost:4545/echo.ts");
await request("http://localhost:4545/not-found");
await request("http://unreachable-host.abc/");
await request("http://localhost:4545/echo.ts", { signal: AbortSignal.abort() });
