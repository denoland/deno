// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/_tls_common.ts");

export const SecureContext = mod.SecureContext;
export const createSecureContext = mod.createSecureContext;
export const translatePeerCertificate = mod.translatePeerCertificate;

export default mod.default;
