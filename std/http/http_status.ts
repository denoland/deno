// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export enum Status {
  Continue = 100, // RFC 7231, 6.2.1
  SwitchingProtocols = 101, // RFC 7231, 6.2.2
  Processing = 102, // RFC 2518, 10.1

  OK = 200, // RFC 7231, 6.3.1
  Created = 201, // RFC 7231, 6.3.2
  Accepted = 202, // RFC 7231, 6.3.3
  NonAuthoritativeInfo = 203, // RFC 7231, 6.3.4
  NoContent = 204, // RFC 7231, 6.3.5
  ResetContent = 205, // RFC 7231, 6.3.6
  PartialContent = 206, // RFC 7233, 4.1
  MultiStatus = 207, // RFC 4918, 11.1
  AlreadyReported = 208, // RFC 5842, 7.1
  IMUsed = 226, // RFC 3229, 10.4.1

  MultipleChoices = 300, // RFC 7231, 6.4.1
  MovedPermanently = 301, // RFC 7231, 6.4.2
  Found = 302, // RFC 7231, 6.4.3
  SeeOther = 303, // RFC 7231, 6.4.4
  NotModified = 304, // RFC 7232, 4.1
  UseProxy = 305, // RFC 7231, 6.4.5
  // _                       = 306, // RFC 7231, 6.4.6 (Unused)
  TemporaryRedirect = 307, // RFC 7231, 6.4.7
  PermanentRedirect = 308, // RFC 7538, 3

  BadRequest = 400, // RFC 7231, 6.5.1
  Unauthorized = 401, // RFC 7235, 3.1
  PaymentRequired = 402, // RFC 7231, 6.5.2
  Forbidden = 403, // RFC 7231, 6.5.3
  NotFound = 404, // RFC 7231, 6.5.4
  MethodNotAllowed = 405, // RFC 7231, 6.5.5
  NotAcceptable = 406, // RFC 7231, 6.5.6
  ProxyAuthRequired = 407, // RFC 7235, 3.2
  RequestTimeout = 408, // RFC 7231, 6.5.7
  Conflict = 409, // RFC 7231, 6.5.8
  Gone = 410, // RFC 7231, 6.5.9
  LengthRequired = 411, // RFC 7231, 6.5.10
  PreconditionFailed = 412, // RFC 7232, 4.2
  RequestEntityTooLarge = 413, // RFC 7231, 6.5.11
  RequestURITooLong = 414, // RFC 7231, 6.5.12
  UnsupportedMediaType = 415, // RFC 7231, 6.5.13
  RequestedRangeNotSatisfiable = 416, // RFC 7233, 4.4
  ExpectationFailed = 417, // RFC 7231, 6.5.14
  Teapot = 418, // RFC 7168, 2.3.3
  MisdirectedRequest = 421, // RFC 7540, 9.1.2
  UnprocessableEntity = 422, // RFC 4918, 11.2
  Locked = 423, // RFC 4918, 11.3
  FailedDependency = 424, // RFC 4918, 11.4
  UpgradeRequired = 426, // RFC 7231, 6.5.15
  PreconditionRequired = 428, // RFC 6585, 3
  TooManyRequests = 429, // RFC 6585, 4
  RequestHeaderFieldsTooLarge = 431, // RFC 6585, 5
  UnavailableForLegalReasons = 451, // RFC 7725, 3

  InternalServerError = 500, // RFC 7231, 6.6.1
  NotImplemented = 501, // RFC 7231, 6.6.2
  BadGateway = 502, // RFC 7231, 6.6.3
  ServiceUnavailable = 503, // RFC 7231, 6.6.4
  GatewayTimeout = 504, // RFC 7231, 6.6.5
  HTTPVersionNotSupported = 505, // RFC 7231, 6.6.6
  VariantAlsoNegotiates = 506, // RFC 2295, 8.1
  InsufficientStorage = 507, // RFC 4918, 11.5
  LoopDetected = 508, // RFC 5842, 7.2
  NotExtended = 510, // RFC 2774, 7
  NetworkAuthenticationRequired = 511 // RFC 6585, 6
}

export const STATUS_TEXT = new Map<Status, string>([
  [Status.Continue, "Continue"],
  [Status.SwitchingProtocols, "Switching Protocols"],
  [Status.Processing, "Processing"],

  [Status.OK, "OK"],
  [Status.Created, "Created"],
  [Status.Accepted, "Accepted"],
  [Status.NonAuthoritativeInfo, "Non-Authoritative Information"],
  [Status.NoContent, "No Content"],
  [Status.ResetContent, "Reset Content"],
  [Status.PartialContent, "Partial Content"],
  [Status.MultiStatus, "Multi-Status"],
  [Status.AlreadyReported, "Already Reported"],
  [Status.IMUsed, "IM Used"],

  [Status.MultipleChoices, "Multiple Choices"],
  [Status.MovedPermanently, "Moved Permanently"],
  [Status.Found, "Found"],
  [Status.SeeOther, "See Other"],
  [Status.NotModified, "Not Modified"],
  [Status.UseProxy, "Use Proxy"],
  [Status.TemporaryRedirect, "Temporary Redirect"],
  [Status.PermanentRedirect, "Permanent Redirect"],

  [Status.BadRequest, "Bad Request"],
  [Status.Unauthorized, "Unauthorized"],
  [Status.PaymentRequired, "Payment Required"],
  [Status.Forbidden, "Forbidden"],
  [Status.NotFound, "Not Found"],
  [Status.MethodNotAllowed, "Method Not Allowed"],
  [Status.NotAcceptable, "Not Acceptable"],
  [Status.ProxyAuthRequired, "Proxy Authentication Required"],
  [Status.RequestTimeout, "Request Timeout"],
  [Status.Conflict, "Conflict"],
  [Status.Gone, "Gone"],
  [Status.LengthRequired, "Length Required"],
  [Status.PreconditionFailed, "Precondition Failed"],
  [Status.RequestEntityTooLarge, "Request Entity Too Large"],
  [Status.RequestURITooLong, "Request URI Too Long"],
  [Status.UnsupportedMediaType, "Unsupported Media Type"],
  [Status.RequestedRangeNotSatisfiable, "Requested Range Not Satisfiable"],
  [Status.ExpectationFailed, "Expectation Failed"],
  [Status.Teapot, "I'm a teapot"],
  [Status.MisdirectedRequest, "Misdirected Request"],
  [Status.UnprocessableEntity, "Unprocessable Entity"],
  [Status.Locked, "Locked"],
  [Status.FailedDependency, "Failed Dependency"],
  [Status.UpgradeRequired, "Upgrade Required"],
  [Status.PreconditionRequired, "Precondition Required"],
  [Status.TooManyRequests, "Too Many Requests"],
  [Status.RequestHeaderFieldsTooLarge, "Request Header Fields Too Large"],
  [Status.UnavailableForLegalReasons, "Unavailable For Legal Reasons"],

  [Status.InternalServerError, "Internal Server Error"],
  [Status.NotImplemented, "Not Implemented"],
  [Status.BadGateway, "Bad Gateway"],
  [Status.ServiceUnavailable, "Service Unavailable"],
  [Status.GatewayTimeout, "Gateway Timeout"],
  [Status.HTTPVersionNotSupported, "HTTP Version Not Supported"],
  [Status.VariantAlsoNegotiates, "Variant Also Negotiates"],
  [Status.InsufficientStorage, "Insufficient Storage"],
  [Status.LoopDetected, "Loop Detected"],
  [Status.NotExtended, "Not Extended"],
  [Status.NetworkAuthenticationRequired, "Network Authentication Required"]
]);
