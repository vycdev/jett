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

func digit(ch string) int64 {
	var pos int64 = 0
	for pos < 10 {
		if at("0123456789", pos) == ch {
			return pos
		}
		pos = (pos + 1)
	}
	return (-1)
}

func alpha(ch string) bool {
	return (contains("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ", ch) && (length(ch) == 1))
}

func alnum(ch string) bool {
	return (alpha(ch) || (digit(ch) >= 0))
}

func solve(text string) bool {
	if length(text) == 0 {
		return false
	}
	var first string = at(text, 0)
	if (!alpha(first)) && (first != "_") {
		return false
	}
	var pos int64 = 1
	for pos < length(text) {
		var ch string = at(text, pos)
		if (!alnum(ch)) && (ch != "_") {
			return false
		}
		pos = (pos + 1)
	}
	return true
}
