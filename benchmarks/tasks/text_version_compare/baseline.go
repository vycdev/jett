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

func uint_value(text string, bound int64) int64 {
	if length(text) == 0 {
		return (-1)
	}
	var value int64 = 0
	var pos int64 = 0
	for pos < length(text) {
		var num int64 = digit(at(text, pos))
		if num < 0 {
			return (-1)
		}
		value = ((value * 10) + num)
		if value > bound {
			return (-1)
		}
		pos = (pos + 1)
	}
	return value
}

func component_end(text string, start int64) int64 {
	var pos int64 = start
	for pos < length(text) {
		if at(text, pos) == "." {
			return pos
		}
		pos = (pos + 1)
	}
	return pos
}

func component_value(text string, start int64, end int64) int64 {
	if start >= length(text) {
		return 0
	}
	return uint_value(part(text, start, end), 999)
}

func solve(left string, right string) int64 {
	var lp int64 = 0
	var rp int64 = 0
	for (lp < length(left)) || (rp < length(right)) {
		var le int64 = component_end(left, lp)
		var re int64 = component_end(right, rp)
		var lv int64 = component_value(left, lp, le)
		var rv int64 = component_value(right, rp, re)
		if lv < rv {
			return (-1)
		}
		if lv > rv {
			return 1
		}
		lp = (le + 1)
		rp = (re + 1)
	}
	return 0
}
