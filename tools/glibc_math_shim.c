// Copyright 2018-2026 the Deno authors. MIT license.

// glibc 2.27 introduced new optimized versions of several libm functions.
// When linking against a glibc >= 2.27 sysroot, the linker picks up the
// GLIBC_2.27 versioned symbols, making the binary incompatible with older
// systems (e.g. AWS Lambda, Amazon Linux 2, Fedora 27).
//
// This shim, combined with --wrap linker flags, redirects these calls to
// the older base versions: GLIBC_2.2.5 on x86_64, GLIBC_2.17 on aarch64.
//
// See: https://github.com/denoland/deno/issues/30432

#if defined(__aarch64__)
#define GLIBC_BASE "GLIBC_2.17"
#elif defined(__x86_64__)
#define GLIBC_BASE "GLIBC_2.2.5"
#else
#error "unsupported architecture for glibc math shim"
#endif

__asm__(".symver __compat_expf, expf@" GLIBC_BASE);
__asm__(".symver __compat_powf, powf@" GLIBC_BASE);
__asm__(".symver __compat_exp2f, exp2f@" GLIBC_BASE);
__asm__(".symver __compat_log2f, log2f@" GLIBC_BASE);
__asm__(".symver __compat_logf, logf@" GLIBC_BASE);

extern float __compat_expf(float);
extern float __compat_powf(float, float);
extern float __compat_exp2f(float);
extern float __compat_log2f(float);
extern float __compat_logf(float);

float __wrap_expf(float x) { return __compat_expf(x); }
float __wrap_powf(float x, float y) { return __compat_powf(x, y); }
float __wrap_exp2f(float x) { return __compat_exp2f(x); }
float __wrap_log2f(float x) { return __compat_log2f(x); }
float __wrap_logf(float x) { return __compat_logf(x); }
