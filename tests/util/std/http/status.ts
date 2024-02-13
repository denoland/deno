// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Contains the {@linkcode STATUS_CODE} object which contains standard HTTP
 * status codes and provides several type guards for handling status codes
 * with type safety.
 *
 * @example
 * ```ts
 * import {
 *   STATUS_CODE,
 *   STATUS_TEXT,
 * } from "https://deno.land/std@$STD_VERSION/http/status.ts";
 *
 * console.log(STATUS_CODE.NotFound); // Returns 404
 * console.log(STATUS_TEXT[STATUS_CODE.NotFound]); // Returns "Not Found"
 * ```
 *
 * @example
 * ```ts
 * import { isErrorStatus } from "https://deno.land/std@$STD_VERSION/http/status.ts";
 *
 * const res = await fetch("https://example.com/");
 *
 * if (isErrorStatus(res.status)) {
 *   // error handling here...
 * }
 * ```
 *
 * @module
 */

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode STATUS_CODE} instead.
 *
 * Standard HTTP status codes.
 */
export enum Status {
  /** RFC 7231, 6.2.1 */
  Continue = 100,
  /** RFC 7231, 6.2.2 */
  SwitchingProtocols = 101,
  /** RFC 2518, 10.1 */
  Processing = 102,
  /** RFC 8297 **/
  EarlyHints = 103,

  /** RFC 7231, 6.3.1 */
  OK = 200,
  /** RFC 7231, 6.3.2 */
  Created = 201,
  /** RFC 7231, 6.3.3 */
  Accepted = 202,
  /** RFC 7231, 6.3.4 */
  NonAuthoritativeInfo = 203,
  /** RFC 7231, 6.3.5 */
  NoContent = 204,
  /** RFC 7231, 6.3.6 */
  ResetContent = 205,
  /** RFC 7233, 4.1 */
  PartialContent = 206,
  /** RFC 4918, 11.1 */
  MultiStatus = 207,
  /** RFC 5842, 7.1 */
  AlreadyReported = 208,
  /** RFC 3229, 10.4.1 */
  IMUsed = 226,

  /** RFC 7231, 6.4.1 */
  MultipleChoices = 300,
  /** RFC 7231, 6.4.2 */
  MovedPermanently = 301,
  /** RFC 7231, 6.4.3 */
  Found = 302,
  /** RFC 7231, 6.4.4 */
  SeeOther = 303,
  /** RFC 7232, 4.1 */
  NotModified = 304,
  /** RFC 7231, 6.4.5 */
  UseProxy = 305,
  /** RFC 7231, 6.4.7 */
  TemporaryRedirect = 307,
  /** RFC 7538, 3 */
  PermanentRedirect = 308,

  /** RFC 7231, 6.5.1 */
  BadRequest = 400,
  /** RFC 7235, 3.1 */
  Unauthorized = 401,
  /** RFC 7231, 6.5.2 */
  PaymentRequired = 402,
  /** RFC 7231, 6.5.3 */
  Forbidden = 403,
  /** RFC 7231, 6.5.4 */
  NotFound = 404,
  /** RFC 7231, 6.5.5 */
  MethodNotAllowed = 405,
  /** RFC 7231, 6.5.6 */
  NotAcceptable = 406,
  /** RFC 7235, 3.2 */
  ProxyAuthRequired = 407,
  /** RFC 7231, 6.5.7 */
  RequestTimeout = 408,
  /** RFC 7231, 6.5.8 */
  Conflict = 409,
  /** RFC 7231, 6.5.9 */
  Gone = 410,
  /** RFC 7231, 6.5.10 */
  LengthRequired = 411,
  /** RFC 7232, 4.2 */
  PreconditionFailed = 412,
  /** RFC 7231, 6.5.11 */
  RequestEntityTooLarge = 413,
  /** RFC 7231, 6.5.12 */
  RequestURITooLong = 414,
  /** RFC 7231, 6.5.13 */
  UnsupportedMediaType = 415,
  /** RFC 7233, 4.4 */
  RequestedRangeNotSatisfiable = 416,
  /** RFC 7231, 6.5.14 */
  ExpectationFailed = 417,
  /** RFC 7168, 2.3.3 */
  Teapot = 418,
  /** RFC 7540, 9.1.2 */
  MisdirectedRequest = 421,
  /** RFC 4918, 11.2 */
  UnprocessableEntity = 422,
  /** RFC 4918, 11.3 */
  Locked = 423,
  /** RFC 4918, 11.4 */
  FailedDependency = 424,
  /** RFC 8470, 5.2 */
  TooEarly = 425,
  /** RFC 7231, 6.5.15 */
  UpgradeRequired = 426,
  /** RFC 6585, 3 */
  PreconditionRequired = 428,
  /** RFC 6585, 4 */
  TooManyRequests = 429,
  /** RFC 6585, 5 */
  RequestHeaderFieldsTooLarge = 431,
  /** RFC 7725, 3 */
  UnavailableForLegalReasons = 451,

  /** RFC 7231, 6.6.1 */
  InternalServerError = 500,
  /** RFC 7231, 6.6.2 */
  NotImplemented = 501,
  /** RFC 7231, 6.6.3 */
  BadGateway = 502,
  /** RFC 7231, 6.6.4 */
  ServiceUnavailable = 503,
  /** RFC 7231, 6.6.5 */
  GatewayTimeout = 504,
  /** RFC 7231, 6.6.6 */
  HTTPVersionNotSupported = 505,
  /** RFC 2295, 8.1 */
  VariantAlsoNegotiates = 506,
  /** RFC 4918, 11.5 */
  InsufficientStorage = 507,
  /** RFC 5842, 7.2 */
  LoopDetected = 508,
  /** RFC 2774, 7 */
  NotExtended = 510,
  /** RFC 6585, 6 */
  NetworkAuthenticationRequired = 511,
}

export const STATUS_CODE = {
  /** RFC 7231, 6.2.1 */
  Continue: 100,
  /** RFC 7231, 6.2.2 */
  SwitchingProtocols: 101,
  /** RFC 2518, 10.1 */
  Processing: 102,
  /** RFC 8297 **/
  EarlyHints: 103,

  /** RFC 7231, 6.3.1 */
  OK: 200,
  /** RFC 7231, 6.3.2 */
  Created: 201,
  /** RFC 7231, 6.3.3 */
  Accepted: 202,
  /** RFC 7231, 6.3.4 */
  NonAuthoritativeInfo: 203,
  /** RFC 7231, 6.3.5 */
  NoContent: 204,
  /** RFC 7231, 6.3.6 */
  ResetContent: 205,
  /** RFC 7233, 4.1 */
  PartialContent: 206,
  /** RFC 4918, 11.1 */
  MultiStatus: 207,
  /** RFC 5842, 7.1 */
  AlreadyReported: 208,
  /** RFC 3229, 10.4.1 */
  IMUsed: 226,

  /** RFC 7231, 6.4.1 */
  MultipleChoices: 300,
  /** RFC 7231, 6.4.2 */
  MovedPermanently: 301,
  /** RFC 7231, 6.4.3 */
  Found: 302,
  /** RFC 7231, 6.4.4 */
  SeeOther: 303,
  /** RFC 7232, 4.1 */
  NotModified: 304,
  /** RFC 7231, 6.4.5 */
  UseProxy: 305,
  /** RFC 7231, 6.4.7 */
  TemporaryRedirect: 307,
  /** RFC 7538, 3 */
  PermanentRedirect: 308,

  /** RFC 7231, 6.5.1 */
  BadRequest: 400,
  /** RFC 7235, 3.1 */
  Unauthorized: 401,
  /** RFC 7231, 6.5.2 */
  PaymentRequired: 402,
  /** RFC 7231, 6.5.3 */
  Forbidden: 403,
  /** RFC 7231, 6.5.4 */
  NotFound: 404,
  /** RFC 7231, 6.5.5 */
  MethodNotAllowed: 405,
  /** RFC 7231, 6.5.6 */
  NotAcceptable: 406,
  /** RFC 7235, 3.2 */
  ProxyAuthRequired: 407,
  /** RFC 7231, 6.5.7 */
  RequestTimeout: 408,
  /** RFC 7231, 6.5.8 */
  Conflict: 409,
  /** RFC 7231, 6.5.9 */
  Gone: 410,
  /** RFC 7231, 6.5.10 */
  LengthRequired: 411,
  /** RFC 7232, 4.2 */
  PreconditionFailed: 412,
  /** RFC 7231, 6.5.11 */
  ContentTooLarge: 413,
  /** RFC 7231, 6.5.12 */
  URITooLong: 414,
  /** RFC 7231, 6.5.13 */
  UnsupportedMediaType: 415,
  /** RFC 7233, 4.4 */
  RangeNotSatisfiable: 416,
  /** RFC 7231, 6.5.14 */
  ExpectationFailed: 417,
  /** RFC 7168, 2.3.3 */
  Teapot: 418,
  /** RFC 7540, 9.1.2 */
  MisdirectedRequest: 421,
  /** RFC 4918, 11.2 */
  UnprocessableEntity: 422,
  /** RFC 4918, 11.3 */
  Locked: 423,
  /** RFC 4918, 11.4 */
  FailedDependency: 424,
  /** RFC 8470, 5.2 */
  TooEarly: 425,
  /** RFC 7231, 6.5.15 */
  UpgradeRequired: 426,
  /** RFC 6585, 3 */
  PreconditionRequired: 428,
  /** RFC 6585, 4 */
  TooManyRequests: 429,
  /** RFC 6585, 5 */
  RequestHeaderFieldsTooLarge: 431,
  /** RFC 7725, 3 */
  UnavailableForLegalReasons: 451,

  /** RFC 7231, 6.6.1 */
  InternalServerError: 500,
  /** RFC 7231, 6.6.2 */
  NotImplemented: 501,
  /** RFC 7231, 6.6.3 */
  BadGateway: 502,
  /** RFC 7231, 6.6.4 */
  ServiceUnavailable: 503,
  /** RFC 7231, 6.6.5 */
  GatewayTimeout: 504,
  /** RFC 7231, 6.6.6 */
  HTTPVersionNotSupported: 505,
  /** RFC 2295, 8.1 */
  VariantAlsoNegotiates: 506,
  /** RFC 4918, 11.5 */
  InsufficientStorage: 507,
  /** RFC 5842, 7.2 */
  LoopDetected: 508,
  /** RFC 2774, 7 */
  NotExtended: 510,
  /** RFC 6585, 6 */
  NetworkAuthenticationRequired: 511,
} as const;

export type StatusCode = typeof STATUS_CODE[keyof typeof STATUS_CODE];

/** A record of all the status codes text. */
export const STATUS_TEXT = {
  [STATUS_CODE.Accepted]: "Accepted",
  [STATUS_CODE.AlreadyReported]: "Already Reported",
  [STATUS_CODE.BadGateway]: "Bad Gateway",
  [STATUS_CODE.BadRequest]: "Bad Request",
  [STATUS_CODE.Conflict]: "Conflict",
  [STATUS_CODE.Continue]: "Continue",
  [STATUS_CODE.Created]: "Created",
  [STATUS_CODE.EarlyHints]: "Early Hints",
  [STATUS_CODE.ExpectationFailed]: "Expectation Failed",
  [STATUS_CODE.FailedDependency]: "Failed Dependency",
  [STATUS_CODE.Forbidden]: "Forbidden",
  [STATUS_CODE.Found]: "Found",
  [STATUS_CODE.GatewayTimeout]: "Gateway Timeout",
  [STATUS_CODE.Gone]: "Gone",
  [STATUS_CODE.HTTPVersionNotSupported]: "HTTP Version Not Supported",
  [STATUS_CODE.IMUsed]: "IM Used",
  [STATUS_CODE.InsufficientStorage]: "Insufficient Storage",
  [STATUS_CODE.InternalServerError]: "Internal Server Error",
  [STATUS_CODE.LengthRequired]: "Length Required",
  [STATUS_CODE.Locked]: "Locked",
  [STATUS_CODE.LoopDetected]: "Loop Detected",
  [STATUS_CODE.MethodNotAllowed]: "Method Not Allowed",
  [STATUS_CODE.MisdirectedRequest]: "Misdirected Request",
  [STATUS_CODE.MovedPermanently]: "Moved Permanently",
  [STATUS_CODE.MultiStatus]: "Multi Status",
  [STATUS_CODE.MultipleChoices]: "Multiple Choices",
  [STATUS_CODE.NetworkAuthenticationRequired]:
    "Network Authentication Required",
  [STATUS_CODE.NoContent]: "No Content",
  [STATUS_CODE.NonAuthoritativeInfo]: "Non Authoritative Info",
  [STATUS_CODE.NotAcceptable]: "Not Acceptable",
  [STATUS_CODE.NotExtended]: "Not Extended",
  [STATUS_CODE.NotFound]: "Not Found",
  [STATUS_CODE.NotImplemented]: "Not Implemented",
  [STATUS_CODE.NotModified]: "Not Modified",
  [STATUS_CODE.OK]: "OK",
  [STATUS_CODE.PartialContent]: "Partial Content",
  [STATUS_CODE.PaymentRequired]: "Payment Required",
  [STATUS_CODE.PermanentRedirect]: "Permanent Redirect",
  [STATUS_CODE.PreconditionFailed]: "Precondition Failed",
  [STATUS_CODE.PreconditionRequired]: "Precondition Required",
  [STATUS_CODE.Processing]: "Processing",
  [STATUS_CODE.ProxyAuthRequired]: "Proxy Auth Required",
  [STATUS_CODE.ContentTooLarge]: "Content Too Large",
  [STATUS_CODE.RequestHeaderFieldsTooLarge]: "Request Header Fields Too Large",
  [STATUS_CODE.RequestTimeout]: "Request Timeout",
  [STATUS_CODE.URITooLong]: "URI Too Long",
  [STATUS_CODE.RangeNotSatisfiable]: "Range Not Satisfiable",
  [STATUS_CODE.ResetContent]: "Reset Content",
  [STATUS_CODE.SeeOther]: "See Other",
  [STATUS_CODE.ServiceUnavailable]: "Service Unavailable",
  [STATUS_CODE.SwitchingProtocols]: "Switching Protocols",
  [STATUS_CODE.Teapot]: "I'm a teapot",
  [STATUS_CODE.TemporaryRedirect]: "Temporary Redirect",
  [STATUS_CODE.TooEarly]: "Too Early",
  [STATUS_CODE.TooManyRequests]: "Too Many Requests",
  [STATUS_CODE.Unauthorized]: "Unauthorized",
  [STATUS_CODE.UnavailableForLegalReasons]: "Unavailable For Legal Reasons",
  [STATUS_CODE.UnprocessableEntity]: "Unprocessable Entity",
  [STATUS_CODE.UnsupportedMediaType]: "Unsupported Media Type",
  [STATUS_CODE.UpgradeRequired]: "Upgrade Required",
  [STATUS_CODE.UseProxy]: "Use Proxy",
  [STATUS_CODE.VariantAlsoNegotiates]: "Variant Also Negotiates",
} as const;

export type StatusText = typeof STATUS_TEXT[keyof typeof STATUS_TEXT];

/** An HTTP status that is a informational (1XX). */
export type InformationalStatus =
  | typeof STATUS_CODE.Continue
  | typeof STATUS_CODE.SwitchingProtocols
  | typeof STATUS_CODE.Processing
  | typeof STATUS_CODE.EarlyHints;

/** An HTTP status that is a success (2XX). */
export type SuccessfulStatus =
  | typeof STATUS_CODE.OK
  | typeof STATUS_CODE.Created
  | typeof STATUS_CODE.Accepted
  | typeof STATUS_CODE.NonAuthoritativeInfo
  | typeof STATUS_CODE.NoContent
  | typeof STATUS_CODE.ResetContent
  | typeof STATUS_CODE.PartialContent
  | typeof STATUS_CODE.MultiStatus
  | typeof STATUS_CODE.AlreadyReported
  | typeof STATUS_CODE.IMUsed;

/** An HTTP status that is a redirect (3XX). */
export type RedirectStatus =
  | typeof STATUS_CODE.MultipleChoices // 300
  | typeof STATUS_CODE.MovedPermanently // 301
  | typeof STATUS_CODE.Found // 302
  | typeof STATUS_CODE.SeeOther // 303
  | typeof STATUS_CODE.UseProxy // 305 - DEPRECATED
  | typeof STATUS_CODE.TemporaryRedirect // 307
  | typeof STATUS_CODE.PermanentRedirect; // 308

/** An HTTP status that is a client error (4XX). */
export type ClientErrorStatus =
  | typeof STATUS_CODE.BadRequest
  | typeof STATUS_CODE.Unauthorized
  | typeof STATUS_CODE.PaymentRequired
  | typeof STATUS_CODE.Forbidden
  | typeof STATUS_CODE.NotFound
  | typeof STATUS_CODE.MethodNotAllowed
  | typeof STATUS_CODE.NotAcceptable
  | typeof STATUS_CODE.ProxyAuthRequired
  | typeof STATUS_CODE.RequestTimeout
  | typeof STATUS_CODE.Conflict
  | typeof STATUS_CODE.Gone
  | typeof STATUS_CODE.LengthRequired
  | typeof STATUS_CODE.PreconditionFailed
  | typeof STATUS_CODE.ContentTooLarge
  | typeof STATUS_CODE.URITooLong
  | typeof STATUS_CODE.UnsupportedMediaType
  | typeof STATUS_CODE.RangeNotSatisfiable
  | typeof STATUS_CODE.ExpectationFailed
  | typeof STATUS_CODE.Teapot
  | typeof STATUS_CODE.MisdirectedRequest
  | typeof STATUS_CODE.UnprocessableEntity
  | typeof STATUS_CODE.Locked
  | typeof STATUS_CODE.FailedDependency
  | typeof STATUS_CODE.UpgradeRequired
  | typeof STATUS_CODE.PreconditionRequired
  | typeof STATUS_CODE.TooManyRequests
  | typeof STATUS_CODE.RequestHeaderFieldsTooLarge
  | typeof STATUS_CODE.UnavailableForLegalReasons;

/** An HTTP status that is a server error (5XX). */
export type ServerErrorStatus =
  | typeof STATUS_CODE.InternalServerError
  | typeof STATUS_CODE.NotImplemented
  | typeof STATUS_CODE.BadGateway
  | typeof STATUS_CODE.ServiceUnavailable
  | typeof STATUS_CODE.GatewayTimeout
  | typeof STATUS_CODE.HTTPVersionNotSupported
  | typeof STATUS_CODE.VariantAlsoNegotiates
  | typeof STATUS_CODE.InsufficientStorage
  | typeof STATUS_CODE.LoopDetected
  | typeof STATUS_CODE.NotExtended
  | typeof STATUS_CODE.NetworkAuthenticationRequired;

/** An HTTP status that is an error (4XX and 5XX). */
export type ErrorStatus = ClientErrorStatus | ServerErrorStatus;

export function isStatus(status: number): status is StatusCode {
  return Object.values(STATUS_CODE).includes(status as StatusCode);
}

/** A type guard that determines if the status code is informational. */
export function isInformationalStatus(
  status: number,
): status is InformationalStatus {
  return isStatus(status) && status >= 100 && status < 200;
}

/** A type guard that determines if the status code is successful. */
export function isSuccessfulStatus(
  status: number,
): status is SuccessfulStatus {
  return isStatus(status) && status >= 200 && status < 300;
}

/** A type guard that determines if the status code is a redirection. */
export function isRedirectStatus(status: number): status is RedirectStatus {
  return isStatus(status) && status >= 300 && status < 400;
}

/** A type guard that determines if the status code is a client error. */
export function isClientErrorStatus(
  status: number,
): status is ClientErrorStatus {
  return isStatus(status) && status >= 400 && status < 500;
}

/** A type guard that determines if the status code is a server error. */
export function isServerErrorStatus(
  status: number,
): status is ServerErrorStatus {
  return isStatus(status) && status >= 500 && status < 600;
}

/** A type guard that determines if the status code is an error. */
export function isErrorStatus(status: number): status is ErrorStatus {
  return isStatus(status) && status >= 400 && status < 600;
}
