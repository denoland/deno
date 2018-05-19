package main

import (
	"net/url"
)

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
