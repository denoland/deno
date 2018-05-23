package main

// For testing
func InitEcho() {
	Sub("echo", func(buf []byte) []byte {
		Pub("echo", buf)
		return nil
	})
}
