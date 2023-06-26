// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/* Copyright Joyent, Inc. and other Node contributors. All rights reserved.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
 * IN THE SOFTWARE.
 */

// This module ports:
// - https://github.com/libuv/libuv/blob/master/src/win/error.c

import * as winErrors from "./_winerror.ts";

export function uvTranslateSysError(sysErrno: number): string {
  switch (sysErrno) {
    case winErrors.ERROR_ACCESS_DENIED:
      return "EACCES";
    case winErrors.ERROR_NOACCESS:
      return "EACCES";
    case winErrors.WSAEACCES:
      return "EACCES";
    // case winErrors.ERROR_ELEVATION_REQUIRED:          return "EACCES";
    case winErrors.ERROR_CANT_ACCESS_FILE:
      return "EACCES";
    case winErrors.ERROR_ADDRESS_ALREADY_ASSOCIATED:
      return "EADDRINUSE";
    case winErrors.WSAEADDRINUSE:
      return "EADDRINUSE";
    case winErrors.WSAEADDRNOTAVAIL:
      return "EADDRNOTAVAIL";
    case winErrors.WSAEAFNOSUPPORT:
      return "EAFNOSUPPORT";
    case winErrors.WSAEWOULDBLOCK:
      return "EAGAIN";
    case winErrors.WSAEALREADY:
      return "EALREADY";
    case winErrors.ERROR_INVALID_FLAGS:
      return "EBADF";
    case winErrors.ERROR_INVALID_HANDLE:
      return "EBADF";
    case winErrors.ERROR_LOCK_VIOLATION:
      return "EBUSY";
    case winErrors.ERROR_PIPE_BUSY:
      return "EBUSY";
    case winErrors.ERROR_SHARING_VIOLATION:
      return "EBUSY";
    case winErrors.ERROR_OPERATION_ABORTED:
      return "ECANCELED";
    case winErrors.WSAEINTR:
      return "ECANCELED";
    case winErrors.ERROR_NO_UNICODE_TRANSLATION:
      return "ECHARSET";
    case winErrors.ERROR_CONNECTION_ABORTED:
      return "ECONNABORTED";
    case winErrors.WSAECONNABORTED:
      return "ECONNABORTED";
    case winErrors.ERROR_CONNECTION_REFUSED:
      return "ECONNREFUSED";
    case winErrors.WSAECONNREFUSED:
      return "ECONNREFUSED";
    case winErrors.ERROR_NETNAME_DELETED:
      return "ECONNRESET";
    case winErrors.WSAECONNRESET:
      return "ECONNRESET";
    case winErrors.ERROR_ALREADY_EXISTS:
      return "EEXIST";
    case winErrors.ERROR_FILE_EXISTS:
      return "EEXIST";
    case winErrors.ERROR_BUFFER_OVERFLOW:
      return "EFAULT";
    case winErrors.WSAEFAULT:
      return "EFAULT";
    case winErrors.ERROR_HOST_UNREACHABLE:
      return "EHOSTUNREACH";
    case winErrors.WSAEHOSTUNREACH:
      return "EHOSTUNREACH";
    case winErrors.ERROR_INSUFFICIENT_BUFFER:
      return "EINVAL";
    case winErrors.ERROR_INVALID_DATA:
      return "EINVAL";
    case winErrors.ERROR_INVALID_NAME:
      return "EINVAL";
    case winErrors.ERROR_INVALID_PARAMETER:
      return "EINVAL";
    // case winErrors.ERROR_SYMLINK_NOT_SUPPORTED:       return "EINVAL";
    case winErrors.WSAEINVAL:
      return "EINVAL";
    case winErrors.WSAEPFNOSUPPORT:
      return "EINVAL";
    case winErrors.ERROR_BEGINNING_OF_MEDIA:
      return "EIO";
    case winErrors.ERROR_BUS_RESET:
      return "EIO";
    case winErrors.ERROR_CRC:
      return "EIO";
    case winErrors.ERROR_DEVICE_DOOR_OPEN:
      return "EIO";
    case winErrors.ERROR_DEVICE_REQUIRES_CLEANING:
      return "EIO";
    case winErrors.ERROR_DISK_CORRUPT:
      return "EIO";
    case winErrors.ERROR_EOM_OVERFLOW:
      return "EIO";
    case winErrors.ERROR_FILEMARK_DETECTED:
      return "EIO";
    case winErrors.ERROR_GEN_FAILURE:
      return "EIO";
    case winErrors.ERROR_INVALID_BLOCK_LENGTH:
      return "EIO";
    case winErrors.ERROR_IO_DEVICE:
      return "EIO";
    case winErrors.ERROR_NO_DATA_DETECTED:
      return "EIO";
    case winErrors.ERROR_NO_SIGNAL_SENT:
      return "EIO";
    case winErrors.ERROR_OPEN_FAILED:
      return "EIO";
    case winErrors.ERROR_SETMARK_DETECTED:
      return "EIO";
    case winErrors.ERROR_SIGNAL_REFUSED:
      return "EIO";
    case winErrors.WSAEISCONN:
      return "EISCONN";
    case winErrors.ERROR_CANT_RESOLVE_FILENAME:
      return "ELOOP";
    case winErrors.ERROR_TOO_MANY_OPEN_FILES:
      return "EMFILE";
    case winErrors.WSAEMFILE:
      return "EMFILE";
    case winErrors.WSAEMSGSIZE:
      return "EMSGSIZE";
    case winErrors.ERROR_FILENAME_EXCED_RANGE:
      return "ENAMETOOLONG";
    case winErrors.ERROR_NETWORK_UNREACHABLE:
      return "ENETUNREACH";
    case winErrors.WSAENETUNREACH:
      return "ENETUNREACH";
    case winErrors.WSAENOBUFS:
      return "ENOBUFS";
    case winErrors.ERROR_BAD_PATHNAME:
      return "ENOENT";
    case winErrors.ERROR_DIRECTORY:
      return "ENOTDIR";
    case winErrors.ERROR_ENVVAR_NOT_FOUND:
      return "ENOENT";
    case winErrors.ERROR_FILE_NOT_FOUND:
      return "ENOENT";
    case winErrors.ERROR_INVALID_DRIVE:
      return "ENOENT";
    case winErrors.ERROR_INVALID_REPARSE_DATA:
      return "ENOENT";
    case winErrors.ERROR_MOD_NOT_FOUND:
      return "ENOENT";
    case winErrors.ERROR_PATH_NOT_FOUND:
      return "ENOENT";
    case winErrors.WSAHOST_NOT_FOUND:
      return "ENOENT";
    case winErrors.WSANO_DATA:
      return "ENOENT";
    case winErrors.ERROR_NOT_ENOUGH_MEMORY:
      return "ENOMEM";
    case winErrors.ERROR_OUTOFMEMORY:
      return "ENOMEM";
    case winErrors.ERROR_CANNOT_MAKE:
      return "ENOSPC";
    case winErrors.ERROR_DISK_FULL:
      return "ENOSPC";
    case winErrors.ERROR_EA_TABLE_FULL:
      return "ENOSPC";
    case winErrors.ERROR_END_OF_MEDIA:
      return "ENOSPC";
    case winErrors.ERROR_HANDLE_DISK_FULL:
      return "ENOSPC";
    case winErrors.ERROR_NOT_CONNECTED:
      return "ENOTCONN";
    case winErrors.WSAENOTCONN:
      return "ENOTCONN";
    case winErrors.ERROR_DIR_NOT_EMPTY:
      return "ENOTEMPTY";
    case winErrors.WSAENOTSOCK:
      return "ENOTSOCK";
    case winErrors.ERROR_NOT_SUPPORTED:
      return "ENOTSUP";
    case winErrors.ERROR_BROKEN_PIPE:
      return "EOF";
    case winErrors.ERROR_PRIVILEGE_NOT_HELD:
      return "EPERM";
    case winErrors.ERROR_BAD_PIPE:
      return "EPIPE";
    case winErrors.ERROR_NO_DATA:
      return "EPIPE";
    case winErrors.ERROR_PIPE_NOT_CONNECTED:
      return "EPIPE";
    case winErrors.WSAESHUTDOWN:
      return "EPIPE";
    case winErrors.WSAEPROTONOSUPPORT:
      return "EPROTONOSUPPORT";
    case winErrors.ERROR_WRITE_PROTECT:
      return "EROFS";
    case winErrors.ERROR_SEM_TIMEOUT:
      return "ETIMEDOUT";
    case winErrors.WSAETIMEDOUT:
      return "ETIMEDOUT";
    case winErrors.ERROR_NOT_SAME_DEVICE:
      return "EXDEV";
    case winErrors.ERROR_INVALID_FUNCTION:
      return "EISDIR";
    case winErrors.ERROR_META_EXPANSION_TOO_LONG:
      return "E2BIG";
    case winErrors.WSAESOCKTNOSUPPORT:
      return "ESOCKTNOSUPPORT";
    default:
      return "UNKNOWN";
  }
}
