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

func closing(ch string) string {
	if ch == ")" {
		return "("
	}
	if ch == "]" {
		return "["
	}
	return "{"
}

func solve(text string) bool {
	var stack string = ""
	var pos int64 = 0
	for pos < length(text) {
		var ch string = at(text, pos)
		if contains("([{", ch) {
			stack = (stack + ch)
		} else {
			if contains(")]}", ch) {
				if length(stack) == 0 {
					return false
				}
				if at(stack, (length(stack)-1)) != closing(ch) {
					return false
				}
				stack = part(stack, 0, (length(stack) - 1))
			}
		}
		pos = (pos + 1)
	}
	return (stack == "")
}
