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
	if patternMatch("aa", "a") != false {
		t.Fatalf("Wrong resullt (1).")
	}
	if patternMatch("aaa[WILDCARD]b", "aaaxsdfdb") != true {
		t.Fatalf("Wrong resullt (2).")
	}
	if patternMatch("aab[WILDCARD]", "xsd") != false {
		t.Fatalf("Wrong resullt (3).")
	}
	if patternMatch("a[WILDCARD]b[WILDCARD]c", "abc") != true {
		t.Fatalf("Wrong resullt (4).")
	}
	if patternMatch("a[WILDCARD]b[WILDCARD]c", "axbc") != true {
		t.Fatalf("Wrong resullt (5).")
	}
	if patternMatch("a[WILDCARD]b[WILDCARD]c", "abxc") != true {
		t.Fatalf("Wrong resullt (6).")
	}
	if patternMatch("a[WILDCARD]b[WILDCARD]c", "axbxc") != true {
		t.Fatalf("Wrong resullt (7).")
	}
	if patternMatch("a[WILDCARD]b[WILDCARD]c", "abcx") != false {
		t.Fatalf("Wrong resullt (8).")
	}
	if patternMatch("a[WILDCARD][WILDCARD]c", "abc") != true {
		t.Fatalf("Wrong resullt (9).")
	}
	if patternMatch("a[WILDCARD][WILDCARD]c", "ac") != true {
		t.Fatalf("Wrong resullt (10).")
	}
}

func TestPatternMatchStackTrace(t *testing.T) {
	if patternMatch(exStackTracePattern, exStackTrace) != true {
		t.Fatalf("Wrong resullt (11).")
	}
}
