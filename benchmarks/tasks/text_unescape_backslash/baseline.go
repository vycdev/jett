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

func decode_escape(ch string) string {
	if ch == "n" {
		return "\n"
	}
	if ch == "t" {
		return "\t"
	}
	if ch == "r" {
		return "\r"
	}
	return ch
}

func solve(text string) TextOutcome {
	var out string = ""
	var pos int64 = 0
	for pos < length(text) {
		var ch string = at(text, pos)
		if ch == "\\" {
			pos = (pos + 1)
			if pos == length(text) {
				return TextRejected{Error: Malformed}
			}
			ch = at(text, pos)
			if !contains("ntr\\\"", ch) {
				return TextRejected{Error: Malformed}
			}
			ch = decode_escape(ch)
		}
		out = (out + ch)
		pos = (pos + 1)
	}
	return TextAccepted{Value: out}
}
