package benchmark

import (
	"strconv"
	"strings"
)

func length(text string) int64 { return int64(len(text)) }
func at(text string, pos int64) string {
	if pos < 0 || pos >= int64(len(text)) {
		return ""
	}
	return text[pos : pos+1]
}
func part(text string, start int64, end int64) string { return text[start:end] }
func contains(text string, needle string) bool        { return strings.Contains(text, needle) }
func lower(text string) string                        { return strings.ToLower(text) }
func upper(text string) string                        { return strings.ToUpper(text) }
func render(value int64) string                       { return strconv.FormatInt(value, 10) }

type TextPair struct {
	First  string
	Second string
}

func solve(text string) TextPair {
	var dot int64 = (-1)
	var pos int64 = 1
	for pos < length(text) {
		if at(text, pos) == "." {
			dot = pos
		}
		pos = (pos + 1)
	}
	if dot < 0 {
		return TextPair{First: text, Second: ""}
	}
	return TextPair{First: part(text, 0, dot), Second: part(text, (dot + 1), length(text))}
}
