// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 *
 * Contains the enum {@linkcode Status} which enumerates standard HTTP status
 * codes and provides several type guards for handling status codes with type
 * safety.
 *
 * @example
 * ```ts
 * import {
 *   Status,
 *   STATUS_TEXT,
 * } from "https://deno.land/std@$STD_VERSION/http/http_status.ts";
 *
 * console.log(Status.NotFound); //=> 404
 * console.log(STATUS_TEXT[Status.NotFound]); //=> "Not Found"
 * ```
 *
 * @example
 * ```ts
 * import { isErrorStatus } from "https://deno.land/std@$STD_VERSION/http/http_status.ts";
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

import * as status from "./status.ts";

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const Status = status.Status;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type Status = status.Status;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const STATUS_TEXT = status.STATUS_TEXT;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type InformationalStatus = status.InformationalStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type SuccessfulStatus = status.SuccessfulStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type RedirectStatus = status.RedirectStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type ClientErrorStatus = status.ClientErrorStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type ServerErrorStatus = status.ServerErrorStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export type ErrorStatus = status.ErrorStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const isInformationalStatus = status.isInformationalStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const isSuccessfulStatus = status.isSuccessfulStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const isRedirectStatus = status.isRedirectStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const isClientErrorStatus = status.isClientErrorStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const isServerErrorStatus = status.isServerErrorStatus;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/http/status.ts} instead.
 */
export const isErrorStatus = status.isErrorStatus;
