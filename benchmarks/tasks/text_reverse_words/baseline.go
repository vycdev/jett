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

func compact(text string) string {
	var out string = ""
	var pending bool = false
	var pos int64 = 0
	for pos < length(text) {
		var ch string = at(text, pos)
		if ch == " " {
			pending = (length(out) > 0)
		} else {
			if pending {
				out = (out + " ")
			}
			out = (out + ch)
			pending = false
		}
		pos = (pos + 1)
	}
	return out
}

func solve(text string) string {
	var clean string = compact(text)
	var out string = ""
	var end int64 = length(clean)
	var pos int64 = (length(clean) - 1)
	for pos >= 0 {
		if at(clean, pos) == " " {
			out = ((out + part(clean, (pos+1), end)) + " ")
			end = pos
		}
		pos = (pos - 1)
	}
	return (out + part(clean, 0, end))
}
