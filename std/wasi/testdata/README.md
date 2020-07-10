# WebAssembly System Interface Test Suite

This directory contains a runtime agnostic test suite for the WebAssembly
System Interface.

The tests are written as standalone WebAssembly command modules compiled
against a specific snapshot of the ABI.

Failure of a test is typically signaled by assertions but may also come from
post conditions specified in the test configuration which is a JSON object
contained at the top of the source code of a test case.

## Prerequisites

- Python
- Rust

## Building

To build all the tests run the following command:

```shell
python build.py
```
