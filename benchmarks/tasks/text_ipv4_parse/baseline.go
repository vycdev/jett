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

type TextError int

const (
	Empty TextError = iota
	Malformed
	Range
)

type TextOutcome interface{ isTextOutcome() }
type TextAccepted struct{ Value int64 }
type TextRejected struct{ Error TextError }

func (TextAccepted) isTextOutcome() {}
func (TextRejected) isTextOutcome() {}

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

func octet(text string) int64 {
	if (length(text) > 1) && (at(text, 0) == "0") {
		return (-1)
	}
	return uint_value(text, 255)
}

func solve(text string) TextOutcome {
	var pos int64 = 0
	var start int64 = 0
	var count int64 = 0
	var value int64 = 0
	for pos <= length(text) {
		if (pos == length(text)) || (at(text, pos) == ".") {
			var num int64 = octet(part(text, start, pos))
			if (num < 0) || (count == 4) {
				return TextRejected{Error: Malformed}
			}
			value = ((value * 256) + num)
			count = (count + 1)
			start = (pos + 1)
		}
		pos = (pos + 1)
	}
	if count != 4 {
		return TextRejected{Error: Malformed}
	}
	return TextAccepted{Value: value}
}
