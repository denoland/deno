// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"fmt"
	"net/url"
	"os"
	"strings"
)

func logDebug(format string, v ...interface{}) {
	// Unless the debug flag is specified, discard logs.
	if *flagDebug {
		fmt.Printf(format+"\n", v...)
	}
}

// exists returns whether the given file or directory exists or not
func exists(path string) bool {
	_, err := os.Stat(path)
	if err == nil {
		return true
	}
	if os.IsNotExist(err) {
		return false
	}
	panic(err)
}

func assert(cond bool, msg string) {
	if !cond {
		panic(msg)
	}
}

func isRemote(filename string) bool {
	u, err := url.Parse(filename)
	check(err)
	return u.IsAbs()
}

func check(e error) {
	if e != nil {
		panic(e)
	}
}

func exitOnError(err error) {
	if err != nil {
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}
}

func async(cb func()) {
	wg.Add(1)
	go func() {
		cb()
		wg.Done()
	}()
}

const wildcard = "[WILDCARD]"

// Matches the pattern string against the text string. The pattern can
// contain "[WILDCARD]" substrings which will match one or more characters.
// Returns true if matched.
func patternMatch(pattern string, text string) bool {
	// Empty pattern only match empty text.
	if len(pattern) == 0 {
		return len(text) == 0
	}

	if pattern == wildcard {
		return true
	}

	parts := strings.Split(pattern, wildcard)

	if len(parts) == 1 {
		return pattern == text
	}

	if strings.HasPrefix(text, parts[0]) {
		text = text[len(parts[0]):]
	} else {
		return false
	}

	for i := 1; i < len(parts); i++ {
		// If the last part is empty, we match.
		if i == len(parts)-1 {
			if parts[i] == "" || parts[i] == "\n" {
				return true
			}
		}
		index := strings.Index(text, parts[i])
		if index < 0 {
			return false
		}
		text = text[index+len(parts[i]):]
	}

	return len(text) == 0
}
