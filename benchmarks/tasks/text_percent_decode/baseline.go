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

func hex_digit(ch string) int64 {
	var pos int64 = 0
	for pos < 16 {
		if at("0123456789abcdef", pos) == lower(ch) {
			return pos
		}
		pos = (pos + 1)
	}
	return (-1)
}

func ascii_print(code int64) string {
	return at(" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~", (code - 32))
}

func solve(text string) TextOutcome {
	var out string = ""
	var pos int64 = 0
	for pos < length(text) {
		var ch string = at(text, pos)
		if ch == "%" {
			if (pos + 2) >= length(text) {
				return TextRejected{Error: Malformed}
			}
			var high int64 = hex_digit(at(text, (pos + 1)))
			var low int64 = hex_digit(at(text, (pos + 2)))
			if (high < 0) || (low < 0) {
				return TextRejected{Error: Malformed}
			}
			var code int64 = ((high * 16) + low)
			if (code < 32) || (code > 126) {
				return TextRejected{Error: Range}
			}
			ch = ascii_print(code)
			pos = (pos + 2)
		}
		out = (out + ch)
		pos = (pos + 1)
	}
	return TextAccepted{Value: out}
}
