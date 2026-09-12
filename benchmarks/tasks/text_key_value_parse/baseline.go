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

func trim_space(text string) string {
	var start int64 = 0
	var end int64 = length(text)
	for (start < end) && (at(text, start) == " ") {
		start = (start + 1)
	}
	for (end > start) && (at(text, (end-1)) == " ") {
		end = (end - 1)
	}
	return part(text, start, end)
}

func solve(text string) TextPair {
	var pos int64 = 0
	for at(text, pos) != "=" {
		pos = (pos + 1)
	}
	return TextPair{First: trim_space(part(text, 0, pos)), Second: trim_space(part(text, (pos + 1), length(text)))}
}
