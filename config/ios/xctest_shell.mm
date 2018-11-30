// Copyright 2016 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#import <UIKit/UIKit.h>
#import <XCTest/XCTest.h>

// For Chrome on iOS we want to run EarlGrey tests (that are XCTests) for all
// our build configurations (Debug, Release, ...). In addition, the symbols
// visibility is configured to private by default. To simplify testing with
// those constraints, our tests are compiled in the TEST_HOST target instead
// of the .xctest bundle that all link against this single test (just there to
// ensure that the bundle is not empty).

@interface XCTestShellEmptyClass : NSObject
@end

@implementation XCTestShellEmptyClass
@end
