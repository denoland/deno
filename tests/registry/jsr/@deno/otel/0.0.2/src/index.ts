// Copyright 2024-2024 the Deno authors. All rights reserved. MIT license.

import { context, trace, metrics } from "npm:@opentelemetry/api@1";

// @ts-ignore Deno.telemetry is not typed yet
const telemetry = Deno.telemetry ?? Deno.tracing;

/**
 * Register `Deno.telemetry` with the OpenTelemetry library.
 */
export function register() {
  context.setGlobalContextManager(telemetry.ContextManager);
  trace.setGlobalTracerProvider(telemetry.TracerProvider);
  metrics.setGlobalMeterProvider(telemetry.MeterProvider);
}
