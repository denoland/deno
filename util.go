package main

import (
	"net/url"
)

func Assert(cond bool, msg string) {
	if !cond {
		panic(msg)
	}
}

func IsRemote(filename string) bool {
	u, err := url.Parse(filename)
	check(err)
	return u.IsAbs()
}

func check(e error) {
	if e != nil {
		panic(e)
	}
}
