// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/* Exact integral types.  */

/* Signed.  */
typedef signed char int8_t;
typedef short int int16_t;
typedef int int32_t;
typedef long int int64_t;

/* Unsigned.  */
typedef unsigned char uint8_t;
typedef unsigned short int uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long int uint64_t;

/* Types for `void *' pointers.  */
typedef long int intptr_t;
typedef unsigned long int uintptr_t;

// https://source.chromium.org/chromium/chromium/src/+/main:v8/include/v8-fast-api-calls.h;l=336
struct FastApiTypedArray {
  uintptr_t length_;
  void* data;
};
