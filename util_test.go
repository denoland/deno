package deno

import (
	"testing"
)

const exStackTrace = `hello
before error
error Error: error
    at foo (/Users/rld/go/src/github.com/ry/deno/testdata/013_async_throw.ts:4:11)
    at eval (/Users/rld/go/src/github.com/ry/deno/testdata/013_async_throw.ts:6:1)
    at Object.eval [as globalEval] (<anonymous>)
    at execute (/main.js:144781:15)
    at FileModule.compileAndRun (/main.js:144678:13)
    at /main.js:145161:13
    at /main.js:15733:13`
const exStackTracePattern = `hello
before error
error Error: error
    at foo ([WILDCARD]testdata/013_async_throw.ts:4:11)
    at eval ([WILDCARD]testdata/013_async_throw.ts:6:1)
    at Object.eval [as globalEval] (<anonymous>)
    at execute (/main.js:[WILDCARD]`

func TestPatternMatch(t *testing.T) {
	if patternMatch("aa", "a") {
		t.Fatalf("Wrong resullt (1).")
	}
	if !patternMatch("aaa[WILDCARD]b", "aaaxsdfdb") {
		t.Fatalf("Wrong resullt (2).")
	}
	if patternMatch("aab[WILDCARD]", "xsd") {
		t.Fatalf("Wrong resullt (3).")
	}
	if !patternMatch("a[WILDCARD]b[WILDCARD]c", "abc") {
		t.Fatalf("Wrong resullt (4).")
	}
	if !patternMatch("a[WILDCARD]b[WILDCARD]c", "axbc") {
		t.Fatalf("Wrong resullt (5).")
	}
	if !patternMatch("a[WILDCARD]b[WILDCARD]c", "abxc") {
		t.Fatalf("Wrong resullt (6).")
	}
	if !patternMatch("a[WILDCARD]b[WILDCARD]c", "axbxc") {
		t.Fatalf("Wrong resullt (7).")
	}
	if patternMatch("a[WILDCARD]b[WILDCARD]c", "abcx") {
		t.Fatalf("Wrong resullt (8).")
	}
	if !patternMatch("a[WILDCARD][WILDCARD]c", "abc") {
		t.Fatalf("Wrong resullt (9).")
	}
	if !patternMatch("a[WILDCARD][WILDCARD]c", "ac") {
		t.Fatalf("Wrong resullt (10).")
	}
}

func TestPatternMatchStackTrace(t *testing.T) {
	if !patternMatch(exStackTracePattern, exStackTrace) {
		t.Fatalf("Wrong resullt (11).")
	}
}
