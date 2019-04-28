// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#ifndef TEST_H_
#define TEST_H_

#include "deno.h"
#include "testing/gtest/include/gtest/gtest.h"

extern deno_snapshot snapshot;  // Loaded in libdeno/test.cc
const deno_buf empty = {nullptr, 0, nullptr, 0};
const deno_snapshot empty_snapshot = {nullptr, 0};

#endif  // TEST_H_
