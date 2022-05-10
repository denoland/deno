# deno_ffi

This crate implements dynamic library ffi.

> Note: This is not how you would typically write Deno runtime extensions. FFI
> is a special case where we want to optimize deserialization of untyped values
> at runtime, to do this we give up on the 'recommended' deno_ops macro.
