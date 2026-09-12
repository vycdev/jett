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

func solve(text string) TextOutcome {
	var best string = ""
	var word string = ""
	var pos int64 = 0
	for pos <= length(text) {
		var ch string = at(text, pos)
		if (ch == " ") || (pos == length(text)) {
			if length(word) > length(best) {
				best = word
			}
			word = ""
		} else {
			word = (word + ch)
		}
		pos = (pos + 1)
	}
	if best == "" {
		return TextRejected{Error: Empty}
	}
	return TextAccepted{Value: best}
}
