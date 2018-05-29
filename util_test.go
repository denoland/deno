package deno

import (
	"testing"
)

func wildcardStr(a string, b string) bool {
	return wildcard([]byte(a), []byte(b))
}

func TestWildcard(t *testing.T) {
	if wildcardStr("aa", "a") != false {
		t.Fatalf("Wrong resullt (1).")
	}
	if wildcardStr("aaa[WILDCARD]b", "aaaxsdfdb") != true {
		t.Fatalf("Wrong resullt (2).")
	}
	if wildcardStr("aab[WILDCARD]", "xsd") != false {
		t.Fatalf("Wrong resullt (3).")
	}
	if wildcardStr("a[WILDCARD]b[WILDCARD]c", "abc") != true {
		t.Fatalf("Wrong resullt (4).")
	}
	if wildcardStr("a[WILDCARD]b[WILDCARD]c", "axbc") != true {
		t.Fatalf("Wrong resullt (5).")
	}
	if wildcardStr("a[WILDCARD]b[WILDCARD]c", "abxc") != true {
		t.Fatalf("Wrong resullt (6).")
	}
	if wildcardStr("a[WILDCARD]b[WILDCARD]c", "axbxc") != true {
		t.Fatalf("Wrong resullt (7).")
	}
	if wildcardStr("a[WILDCARD]b[WILDCARD]c", "abcx") != false {
		t.Fatalf("Wrong resullt (8).")
	}
	if wildcardStr("a[WILDCARD][WILDCARD]c", "abc") != true {
		t.Fatalf("Wrong resullt (9).")
	}
	if wildcardStr("a[WILDCARD][WILDCARD]c", "ac") != true {
		t.Fatalf("Wrong resullt (10).")
	}
}
