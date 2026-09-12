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

func solve(text string, needle string) int64 {
	var count int64 = 0
	var pos int64 = 0
	for (pos + length(needle)) <= length(text) {
		if part(text, pos, (pos+length(needle))) == needle {
			count = (count + 1)
		}
		pos = (pos + 1)
	}
	return count
}
