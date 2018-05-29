// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package main

// For testing
func InitEcho() {
	Sub("echo", func(buf []byte) []byte {
		Pub("echo", buf)
		return nil
	})
}
