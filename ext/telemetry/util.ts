// Copyright 2018-2026 the Deno authors. MIT license.

import { internals, primordials } from "ext:core/mod.js";
import type { Span } from "ext:deno_telemetry/telemetry.ts";

const { String, StringPrototypeSlice } = primordials;

export function updateSpanFromRequest(span: Span, request: Request) {
  span.updateName(request.method);

  span.setAttribute("http.request.method", request.method);
  const url = new URL(request.url);
  span.setAttribute("url.full", request.url);
  span.setAttribute(
    "url.scheme",
    StringPrototypeSlice(url.protocol, 0, -1),
  );
  span.setAttribute("url.path", url.pathname);
  span.setAttribute("url.query", StringPrototypeSlice(url.search, 1));
}

function setResponseAttributes(
  span: Span,
  response: Response,
  errorThreshold: number,
) {
  span.setAttribute(
    "http.response.status_code",
    String(response.status),
  );
  if (response.status >= errorThreshold) {
    span.setAttribute("error.type", String(response.status));
    span.setStatus({ code: 2, message: response.statusText });
  }
}

// Per OTel HTTP semantic conventions, client spans should have ERROR status
// for all >= 400 responses.
export function updateSpanFromClientResponse(
  span: Span,
  response: Response,
) {
  setResponseAttributes(span, response, 400);
}

// Per OTel HTTP semantic conventions, server spans should only have ERROR
// status for 5xx responses. 4xx responses are client errors, not server
// errors.
export function updateSpanFromServerResponse(
  span: Span,
  response: Response,
) {
  setResponseAttributes(span, response, 500);
}

// deno-lint-ignore no-explicit-any
export function updateSpanFromError(span: Span, error: any) {
  const errorType = error.name ?? "Error";
  span.setAttribute("error.type", errorType);
  span.setAttribute("exception.type", errorType);
  if (error.message != null) {
    span.setAttribute("exception.message", error.message);
  }
  if (error.stack != null) {
    span.setAttribute("exception.stacktrace", error.stack);
  }
  span.setStatus({ code: 2, message: error.message ?? String(error) });
}

internals.__telemetryUtil = {
  updateSpanFromClientResponse,
  updateSpanFromError,
  updateSpanFromRequest,
};
