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

func solve(text string, pattern string) bool {
	var ti int64 = 0
	var pi int64 = 0
	var star int64 = (-1)
	var retry int64 = 0
	for ti < length(text) {
		var ch string = at(pattern, pi)
		if (pi < length(pattern)) && ((ch == "?") || (ch == at(text, ti))) {
			ti = (ti + 1)
			pi = (pi + 1)
		} else {
			if ch == "*" {
				star = pi
				retry = ti
				pi = (pi + 1)
			} else {
				if star < 0 {
					return false
				}
				retry = (retry + 1)
				ti = retry
				pi = (star + 1)
			}
		}
	}
	for at(pattern, pi) == "*" {
		pi = (pi + 1)
	}
	return (pi == length(pattern))
}
