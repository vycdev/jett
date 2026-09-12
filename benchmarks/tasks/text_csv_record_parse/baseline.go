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
func new_words() []string                             { return []string{} }
func push(values []string, value string) []string     { return append(values, value) }

type TextError int

const (
	Empty TextError = iota
	Malformed
	Range
)

type TextOutcome interface{ isTextOutcome() }
type TextAccepted struct{ Value []string }
type TextRejected struct{ Error TextError }

func (TextAccepted) isTextOutcome() {}
func (TextRejected) isTextOutcome() {}

func quoted_end(text string, start int64) int64 {
	var pos int64 = (start + 1)
	for pos < length(text) {
		if at(text, pos) == "\"" {
			if at(text, (pos+1)) == "\"" {
				pos = (pos + 2)
			} else {
				return (pos + 1)
			}
		} else {
			pos = (pos + 1)
		}
	}
	return (-1)
}

func field_end(text string, start int64) int64 {
	if at(text, start) == "\"" {
		return quoted_end(text, start)
	}
	var pos int64 = start
	for (pos < length(text)) && (at(text, pos) != ",") {
		if at(text, pos) == "\"" {
			return (-1)
		}
		pos = (pos + 1)
	}
	return pos
}

func decoded_field(text string, start int64, end int64) string {
	if at(text, start) != "\"" {
		return part(text, start, end)
	}
	var out string = ""
	var pos int64 = (start + 1)
	for pos < (end - 1) {
		var ch string = at(text, pos)
		out = (out + ch)
		if ch == "\"" {
			pos = (pos + 1)
		}
		pos = (pos + 1)
	}
	return out
}

func solve(text string) TextOutcome {
	var pos int64 = 0
	var fields []string = new_words()
	for pos <= length(text) {
		var end int64 = field_end(text, pos)
		if end < 0 {
			return TextRejected{Error: Malformed}
		}
		fields = push(fields, decoded_field(text, pos, end))
		if end == length(text) {
			return TextAccepted{Value: fields}
		}
		if at(text, end) != "," {
			return TextRejected{Error: Malformed}
		}
		pos = (end + 1)
	}
	return TextRejected{Error: Malformed}
}
