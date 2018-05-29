// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"fmt"
	"net/url"
	"os"
	"bytes"
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

var WILDCARD = []byte("[WILDCARD]")

func wildcard(pattern []byte, text []byte) bool {
	// Empty pattern only match empty text.
	if len(pattern) == 0 {
		return len(text) == 0
	}
	
	if bytes.Equal(pattern, WILDCARD) {
		return true
	}

	parts := bytes.Split(pattern, WILDCARD)
	numParts := len(parts)

	if numParts == 1 {
		return bytes.Equal(pattern, text)
	}

	if bytes.HasPrefix(text, parts[0]) {
		text = text[len(parts[0]):]
	} else {
		return false
	}

	// *parts[i]
	for i := 1; i < numParts; i++ {
		index := bytes.Index(text, parts[i])
		if index < 0 {
			return false
		}
		text = text[index + len(parts[i]):]
	}

	return len(text) == 0
}
