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

func compact(text string) string {
	var out string = ""
	var pending bool = false
	var pos int64 = 0
	for pos < length(text) {
		var ch string = at(text, pos)
		if ch == " " {
			pending = (length(out) > 0)
		} else {
			if pending {
				out = (out + " ")
			}
			out = (out + ch)
			pending = false
		}
		pos = (pos + 1)
	}
	return out
}

func append_word(out string, word string, used int64, width int64) string {
	if out == "" {
		return word
	}
	if ((used + 1) + length(word)) <= width {
		return ((out + " ") + word)
	}
	return ((out + "\n") + word)
}

func solve(text string, width int64) string {
	var clean string = compact(text)
	var out string = ""
	var used int64 = 0
	var start int64 = 0
	var pos int64 = 0
	for pos <= length(clean) {
		if (at(clean, pos) == " ") || (pos == length(clean)) {
			var word string = part(clean, start, pos)
			out = append_word(out, word, used, width)
			if (used == 0) || (((used + 1) + length(word)) > width) {
				used = length(word)
			} else {
				used = ((used + 1) + length(word))
			}
			start = (pos + 1)
		}
		pos = (pos + 1)
	}
	return out
}
