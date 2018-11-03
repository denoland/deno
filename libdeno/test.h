// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef TEST_H
#define TEST_H

#include "deno.h"
#include "testing/gtest/include/gtest/gtest.h"

extern deno_buf snapshot;  // Loaded in libdeno/test.cc
const deno_buf empty = {nullptr, 0, nullptr, 0};

#endif  // TEST_H
