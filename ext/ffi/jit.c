// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
unsigned char deno_ffi_u8(void* info, int i);
unsigned short deno_ffi_u16(void* info, int i);
unsigned int deno_ffi_u32(void* info, int i);
unsigned long deno_ffi_u64(void* info, int i);
short deno_ffi_i16(void* info, int i);
char deno_ffi_i8(void* info, int i);
int deno_ffi_i32(void* info, int i);
long deno_ffi_i64(void* info, int i);
float deno_ffi_f32(void* info, int i);
double deno_ffi_f64(void* info, int i);
void* deno_ffi_pointer(void* info, int i);
void* deno_ffi_function(void* info, int i);

void deno_rv_u8(void* info, unsigned char v);
void deno_rv_u16(void* info, unsigned short v);
void deno_rv_u32(void* info, unsigned int v);
void deno_rv_u64(void* info, unsigned long v);
void deno_rv_i8(void* info, char v);
void deno_rv_i16(void* info, short v);
void deno_rv_i32(void* info, int v);
void deno_rv_i64(void* info, long v);
void deno_rv_f32(void* info, float v);
void deno_rv_f64(void* info, double v);
void deno_rv_pointer(void* info, void* v);
void deno_rv_function(void* info, void* v);