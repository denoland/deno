let receivedConfiguredRequest = false;
const server = Deno.serve(
  { hostname: "127.0.0.1", port: 0, onListen() {} },
  (request) => {
    const url = new URL(request.url);
    receivedConfiguredRequest ||= request.method === "GET" &&
      url.pathname.startsWith("/objects/") &&
      request.headers.get("authorization") === "Bearer launcher-token";
    return new Response("configured cache endpoint");
  },
);

const tempDir = Deno.makeTempDirSync();
const envFilePath = `${tempDir}/startup.env`;
const traceDir = `${tempDir}/webgpu-trace`;
const endpoint = `http://127.0.0.1:${server.addr.port}`;
Deno.writeTextFileSync(
  envFilePath,
  [
    `DENO_CACHE_LSC_ENDPOINT=${endpoint},launcher-token`,
    `DENO_WEBGPU_TRACE=${traceDir}`,
  ].join("\n"),
);

try {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--allow-env=DENO_WEBGPU_TRACE",
      `--env-file=${envFilePath}`,
      "main.ts",
      traceDir,
    ],
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  });
  const output = await command.output();
  if (!output.success) {
    throw new Error(new TextDecoder().decode(output.stderr));
  }
  if (!receivedConfiguredRequest) {
    throw new Error("configured cache endpoint was not used");
  }
  console.log(new TextDecoder().decode(output.stdout).trim());
  console.log("configured endpoint used");
} finally {
  await server.shutdown();
  Deno.removeSync(tempDir, { recursive: true });
}
