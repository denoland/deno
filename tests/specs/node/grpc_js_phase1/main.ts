// Copyright 2018-2026 the Deno authors. MIT license.
// Test gRPC unary, server-streaming, unknown-method, and client-cancel
// using @grpc/grpc-js self-hosted server+client.

import * as grpc from "npm:@grpc/grpc-js@1.14.3";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

// ─── Codec helpers ───────────────────────────────────────────────

interface HelloRequest {
  name: string;
}
interface HelloReply {
  message: string;
}
interface StreamReply {
  seq: number;
}

const encode = <T>(value: T): Buffer =>
  Buffer.from(JSON.stringify(value), "utf-8");

const decode = <T>(value: Buffer): T => JSON.parse(value.toString("utf-8"));

// ─── Service definitions ─────────────────────────────────────────

type GreeterHandlers = {
  SayHello: grpc.handleUnaryCall<HelloRequest, HelloReply>;
  StreamHellos: grpc.handleServerStreamingCall<HelloRequest, StreamReply>;
};

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
  StreamHellos: {
    path: "/test.Greeter/StreamHellos",
    requestStream: false,
    responseStream: true,
    requestSerialize: encode<HelloRequest>,
    requestDeserialize: decode<HelloRequest>,
    responseSerialize: encode<StreamReply>,
    responseDeserialize: decode<StreamReply>,
  },
};

// ─── Client constructor ──────────────────────────────────────────

type GreeterClient = grpc.Client & {
  SayHello(
    request: HelloRequest,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  StreamHellos(request: HelloRequest): grpc.ClientReadableStream<StreamReply>;
};

const GreeterClientCtor = grpc.makeGenericClientConstructor(
  GreeterService,
  "Greeter",
) as unknown as {
  new (address: string, creds: grpc.ChannelCredentials): GreeterClient;
};

// ─── Server helpers ──────────────────────────────────────────────

async function startServer(): Promise<{ server: grpc.Server; port: number }> {
  const server = new grpc.Server();

  const impl: GreeterHandlers = {
    SayHello(call, callback) {
      callback(null, { message: `Hello, ${call.request.name}` });
    },
    StreamHellos(call) {
      for (let i = 0; i < 3; i++) {
        call.write({ seq: i });
      }
      call.end();
    },
  };

  server.addService(GreeterService, impl);

  const port = await new Promise<number>((resolve, reject) => {
    server.bindAsync(
      "127.0.0.1:0",
      grpc.ServerCredentials.createInsecure(),
      (err, boundPort) => {
        if (err) return reject(err);
        resolve(boundPort);
      },
    );
  });

  return { server, port };
}

function createClient(port: number): GreeterClient {
  return new GreeterClientCtor(
    `127.0.0.1:${port}`,
    grpc.credentials.createInsecure(),
  );
}

async function stopServer(server: grpc.Server): Promise<void> {
  await new Promise<void>((resolve) => server.tryShutdown(() => resolve()));
}

// ─── Test: Unary OK ──────────────────────────────────────────────

async function testUnaryOk(): Promise<void> {
  const { server, port } = await startServer();
  const client = createClient(port);

  try {
    await new Promise<void>((resolve, reject) => {
      const call = client.SayHello({ name: "Deno" }, (err, response) => {
        if (err) return reject(err);
        try {
          assert.equal(response?.message, "Hello, Deno");
        } catch (e) {
          return reject(e);
        }
      });
      call.on("status", (status) => {
        try {
          assert.equal(status.code, grpc.status.OK);
          resolve();
        } catch (e) {
          reject(e);
        }
      });
      call.on("error", reject);
    });
    console.log("UNARY_OK");
  } finally {
    client.close();
    await stopServer(server);
  }
}

// ─── Test: Server-streaming OK ───────────────────────────────────

async function testServerStreamOk(): Promise<void> {
  const { server, port } = await startServer();
  const client = createClient(port);

  try {
    await new Promise<void>((resolve, reject) => {
      const stream = client.StreamHellos({ name: "Deno" });
      const messages: StreamReply[] = [];

      stream.on("data", (msg: StreamReply) => {
        messages.push(msg);
      });

      stream.on("status", (status) => {
        try {
          assert.equal(status.code, grpc.status.OK);
          assert.equal(messages.length, 3);
          assert.deepStrictEqual(messages, [
            { seq: 0 },
            { seq: 1 },
            { seq: 2 },
          ]);
          resolve();
        } catch (e) {
          reject(e);
        }
      });

      stream.on("error", reject);
    });
    console.log("STREAM_OK");
  } finally {
    client.close();
    await stopServer(server);
  }
}

// ─── Test: Unknown method → UNIMPLEMENTED ────────────────────────

async function testUnknownMethodUnimplemented(): Promise<void> {
  const { server, port } = await startServer();
  const client = createClient(port);
  let unknownClient:
    | (grpc.Client & {
      NoSuchMethod(
        request: unknown,
        callback: grpc.requestCallback<unknown>,
      ): grpc.ClientUnaryCall;
    })
    | undefined;

  try {
    // Build a client for a method that the server doesn't implement
    const unknownService: grpc.ServiceDefinition = {
      NoSuchMethod: {
        path: "/test.Greeter/NoSuchMethod",
        requestStream: false,
        responseStream: false,
        requestSerialize: encode,
        requestDeserialize: decode,
        responseSerialize: encode,
        responseDeserialize: decode,
      },
    };
    const UnknownClientCtor = grpc.makeGenericClientConstructor(
      unknownService,
      "Unknown",
    ) as unknown as {
      new (
        address: string,
        creds: grpc.ChannelCredentials,
      ): grpc.Client & {
        NoSuchMethod(
          request: unknown,
          callback: grpc.requestCallback<unknown>,
        ): grpc.ClientUnaryCall;
      };
    };

    unknownClient = new UnknownClientCtor(
      `127.0.0.1:${port}`,
      grpc.credentials.createInsecure(),
    );

    await new Promise<void>((resolve, reject) => {
      unknownClient!.NoSuchMethod({}, (err, _response) => {
        try {
          assert.ok(err, "Expected an error for unknown method");
          assert.equal(err!.code, grpc.status.UNIMPLEMENTED);
          resolve();
        } catch (e) {
          reject(e);
        }
      });
    });
    console.log("UNKNOWN_OK");
  } finally {
    unknownClient?.close();
    client.close();
    await stopServer(server);
  }
}

// ─── Test: Client cancel ─────────────────────────────────────────

async function testClientCancel(): Promise<void> {
  const { server, port } = await startServer();
  const client = createClient(port);

  try {
    await new Promise<void>((resolve, reject) => {
      const call = client.SayHello({ name: "CancelMe" }, (err, _response) => {
        try {
          assert.ok(err, "Expected a cancellation error");
          assert.equal(err!.code, grpc.status.CANCELLED);
          resolve();
        } catch (e) {
          reject(e);
        }
      });
      // Cancel immediately
      call.cancel();
    });
    console.log("CANCEL_OK");
  } finally {
    client.close();
    await stopServer(server);
  }
}

// ─── Main ────────────────────────────────────────────────────────

async function main(): Promise<void> {
  await testUnaryOk();
  await testServerStreamOk();
  await testUnknownMethodUnimplemented();
  await testClientCancel();
  console.log("DONE");
}

main().catch((error) => {
  console.error(error);
  Deno.exit(1);
});
