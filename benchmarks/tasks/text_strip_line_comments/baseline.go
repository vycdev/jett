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

func quote_mode(mode int64, ch string) int64 {
	if mode == 2 {
		return 1
	}
	if (mode == 1) && (ch == "\\") {
		return 2
	}
	if ch == "\"" {
		return (1 - mode)
	}
	return mode
}

func solve(text string) string {
	var out string = ""
	var mode int64 = 0
	var comment bool = false
	var pos int64 = 0
	for pos < length(text) {
		var ch string = at(text, pos)
		if comment {
			if ch == "\n" {
				comment = false
				out = (out + ch)
			}
		} else {
			if (mode == 0) && (ch == "#") {
				comment = true
			} else {
				out = (out + ch)
				mode = quote_mode(mode, ch)
			}
		}
		pos = (pos + 1)
	}
	return out
}
