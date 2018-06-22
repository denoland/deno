// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"strings"
)

type codeChunck struct {
	code       string
	chunckType chunckType
}

type chunckType int

const (
	importNode = 0
	other      = 1
)

func TopLevelAwait(sourceCode string) string {
	var transformedCode string

	for _, c := range split(sourceCode) {
		// TODO: "await" could be inside an async function,
		// part of a string, comment or an identifier
		if c.chunckType == other && strings.Contains(c.code, "await") {
			transformedCode += "\n(async () => {\n" + strings.TrimFunc(c.code, isWS) + "\n})();\n"
		} else {
			transformedCode += "\n" + strings.TrimFunc(c.code, isWS) + ";\n"
		}
	}

	return strings.TrimFunc(transformedCode, isWS)
}

func split(sourceCode string) []codeChunck {
	var cursor int
	var node codeChunck
	end := len(sourceCode) - 1
	chuncks := []codeChunck{}

	for cursor <= end {
		// TODO: this only keeps "import"s out of the async iife, but
		// exports, functions and classes, and exported identifiers
		// should be outside too
		if strings.HasPrefix(sourceCode[cursor:], "import ") {
			node, cursor = getImportNode(sourceCode, cursor)
		} else {
			node, cursor = getNotImportNode(sourceCode, cursor)
		}

		if l := len(chuncks); l > 0 && chuncks[l-1].chunckType == node.chunckType {
			chuncks[l-1].code += node.code
		} else {
			chuncks = append(chuncks, node)
		}
	}

	return chuncks
}

func getNotImportNode(sourceCode string, cursor int) (codeChunck, int) {
	end := len(sourceCode) - 1
	before := cursor

	// TODO: here "import " could be in a comment or part of an identifier
	for cursor <= end && !strings.HasPrefix(sourceCode[cursor:], "import ") {
		cursor++
	}

	return codeChunck{
		code:       sourceCode[before:cursor],
		chunckType: other,
	}, cursor
}

func getImportNode(sourceCode string, cursor int) (codeChunck, int) {
	var quote int
	end := len(sourceCode) - 1
	before := cursor

	for cursor <= end {
		char := sourceCode[cursor]

		if char == '"' || char == '\'' {
			quote++
		}
		cursor++
		if quote == 2 {
			cursor = whiteSpaceOrSemiCount(sourceCode, cursor)
			return codeChunck{
				code:       sourceCode[before:cursor],
				chunckType: importNode,
			}, cursor
		}
	}

	return codeChunck{
		code:       sourceCode[before:cursor],
		chunckType: importNode,
	}, cursor
}

func whiteSpaceOrSemiCount(sourceCode string, cursor int) int {
	end := len(sourceCode) - 1

	for cursor <= end {
		char := sourceCode[cursor]

		if isWS(rune(char)) || char == ';' {
			cursor++
		} else {
			return cursor
		}
	}

	return cursor
}

func isWS(c rune) bool {
	return c == '\n' || c == ' ' || c == '\t'
}
