import * as grpc from "npm:@grpc/grpc-js@1.14.3";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";

interface HelloRequest {
  name: string;
}

interface HelloReply {
  message: string;
}

type GreeterHandlers = {
  SayHello: grpc.handleUnaryCall<HelloRequest, HelloReply>;
};

type GreeterClient = grpc.Client & {
  SayHello(
    request: HelloRequest,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  SayHello(
    request: HelloRequest,
    options: grpc.CallOptions,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
};

const encode = <T>(value: T): Buffer =>
  Buffer.from(JSON.stringify(value), "utf-8");
const decode = <T>(value: Buffer): T => JSON.parse(value.toString("utf-8"));

const ROOT_CA = readFileSync(
  new URL("../../../testdata/tls/RootCA.crt", import.meta.url),
);
const SERVER_CERT = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const SERVER_KEY = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);
const CLIENT_CERT = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const CLIENT_KEY = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const GreeterService: grpc.ServiceDefinition<GreeterHandlers> = {
  SayHello: {
    path: "/test.Greeter/SayHello",
    requestStream: false,
    responseStream: false,
    requestSerialize: encode<HelloRequest>,
    requestDeserialize: decode<HelloRequest>,
    responseSerialize: encode<HelloReply>,
    responseDeserialize: decode<HelloReply>,
  },
};

const GreeterClientCtor = grpc.makeGenericClientConstructor(
  GreeterService,
  "GreeterMtls",
) as unknown as {
  new (
    address: string,
    creds: grpc.ChannelCredentials,
    options?: grpc.ChannelOptions,
  ): GreeterClient;
};

async function startServer(): Promise<{ server: grpc.Server; port: number }> {
  const server = new grpc.Server();

  server.addService(GreeterService, {
    SayHello(
      call: grpc.ServerUnaryCall<HelloRequest, HelloReply>,
      callback: grpc.sendUnaryData<HelloReply>,
    ) {
      callback(null, { message: `Hello, ${call.request.name}` });
    },
  });

  const port = await new Promise<number>((resolve, reject) => {
    server.bindAsync(
      "localhost:0",
      grpc.ServerCredentials.createSsl(ROOT_CA, [{
        private_key: SERVER_KEY,
        cert_chain: SERVER_CERT,
      }], true),
      (error, boundPort) => {
        if (error) return reject(error);
        resolve(boundPort);
      },
    );
  });

  return { server, port };
}

function createClient(
  port: number,
  creds: grpc.ChannelCredentials,
  targetNameOverride = "localhost",
): GreeterClient {
  return new GreeterClientCtor(`localhost:${port}`, creds, {
    "grpc.keepalive_time_ms": 10,
    "grpc.keepalive_timeout_ms": 100,
    "grpc.keepalive_permit_without_calls": 1,
    ...(targetNameOverride
      ? { "grpc.ssl_target_name_override": targetNameOverride }
      : {}),
  });
}

async function stopServer(server: grpc.Server): Promise<void> {
  await new Promise<void>((resolve) => server.tryShutdown(() => resolve()));
}

function unary(
  client: GreeterClient,
  request: HelloRequest,
  deadlineMs = 2_000,
): Promise<HelloReply> {
  return new Promise<HelloReply>((resolve, reject) => {
    client.SayHello(
      request,
      { deadline: Date.now() + deadlineMs },
      (error, response) => {
        if (error) return reject(error);
        if (!response) return reject(new Error("missing unary response"));
        resolve(response);
      },
    );
  });
}

const { server, port } = await startServer();
const goodClient = createClient(
  port,
  grpc.credentials.createSsl(ROOT_CA, CLIENT_KEY, CLIENT_CERT),
);

try {
  const goodReply = await unary(goodClient, { name: "mTLS" });
  assert.equal(goodReply.message, "Hello, mTLS");
  console.log("MTLS_AUTH_OK");

  const noClientCert = createClient(port, grpc.credentials.createSsl(ROOT_CA));
  const noClientCertResult = await new Promise<{
    error: grpc.ServiceError | null;
    response: HelloReply | null;
  }>((resolve) => {
    noClientCert.SayHello(
      { name: "missing-client-cert" },
      { deadline: Date.now() + 1_500 },
      (error, response) => {
        resolve({ error, response: response ?? null });
      },
    );
  });
  noClientCert.close();
  assert.ok(noClientCertResult.error, "expected no-cert client to be rejected");
  assert.equal(noClientCertResult.error.code, grpc.status.UNAVAILABLE);
  assert.equal(noClientCertResult.response, null);
  console.log("MTLS_NO_CERT_REJECT_OK");

  console.log("MTLS_DONE");
} finally {
  goodClient.close();
  await stopServer(server);
}
