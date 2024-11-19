// Copyright 2024-2024 the Deno authors. All rights reserved. MIT license.

import { context } from "npm:@opentelemetry/api@1";
import {
  BasicTracerProvider,
  SimpleSpanProcessor,
} from "npm:@opentelemetry/sdk-trace-base@1";

// @ts-ignore Deno.telemetry is not typed yet
const telemetry = Deno.telemetry ?? Deno.tracing;

let COUNTER = 1;

/**
 * Register `Deno.telemetry` with the OpenTelemetry library.
 */
export function register() {
  context.setGlobalContextManager(
    new telemetry.ContextManager() ?? telemetry.ContextManager(),
  );

  const provider = new BasicTracerProvider({
    idGenerator: Deno.env.get("DENO_UNSTABLE_OTEL_DETERMINISTIC") === "1" ? {
      generateSpanId() {
        return "1" + String(COUNTER++).padStart(15, "0");
      },
      generateTraceId() {
        return "1" + String(COUNTER++).padStart(31, "0");
      }
    } : undefined
  });

  // @ts-ignore Deno.tracing is not typed yet
  const exporter = new telemetry.SpanExporter();
  provider.addSpanProcessor(new SimpleSpanProcessor(exporter));

  provider.register();
}
