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

func boundary(text string, pos int64) bool {
	if pos == 0 {
		return false
	}
	var ch string = at(text, pos)
	if !contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ", ch) {
		return false
	}
	var prev string = at(text, (pos - 1))
	if contains("abcdefghijklmnopqrstuvwxyz0123456789", prev) {
		return true
	}
	return (contains("abcdefghijklmnopqrstuvwxyz", at(text, (pos+1))) && ((pos + 1) < length(text)))
}

func solve(text string) string {
	var out string = ""
	var pos int64 = 0
	for pos < length(text) {
		if boundary(text, pos) {
			out = (out + "_")
		}
		out = (out + lower(at(text, pos)))
		pos = (pos + 1)
	}
	return out
}
