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
type TextAccepted struct{ Value string }
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

func repeat_letter(ch string, count int64) string {
	var out string = ""
	var pos int64 = 0
	for pos < count {
		out = (out + ch)
		pos = (pos + 1)
	}
	return out
}

func solve(text string) TextOutcome {
	var out string = ""
	var pos int64 = 0
	for pos < length(text) {
		var start int64 = pos
		for (pos < length(text)) && (digit(at(text, pos)) >= 0) {
			pos = (pos + 1)
		}
		var count int64 = uint_value(part(text, start, pos), 100)
		if (count <= 0) || (at(text, start) == "0") {
			return TextRejected{Error: Malformed}
		}
		var ch string = at(text, pos)
		if (pos == length(text)) || (!contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ", ch)) {
			return TextRejected{Error: Malformed}
		}
		if (length(out) + count) > 100 {
			return TextRejected{Error: Range}
		}
		out = (out + repeat_letter(ch, count))
		pos = (pos + 1)
	}
	return TextAccepted{Value: out}
}
