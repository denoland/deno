// Copyright 2018-2025 the Deno authors. MIT license.

export const kAuthority = Symbol("authority");
export const kSensitiveHeaders = Symbol("sensitiveHeaders");
export const kSocket = Symbol("socket");
export const kProtocol = Symbol("protocol");
export const kProxySocket = Symbol("proxySocket");
export const kRequest = Symbol("request");

export default {
  kAuthority,
  kSensitiveHeaders,
  kSocket,
  kProtocol,
  kProxySocket,
  kRequest,
};
