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

func parent_path(text string) string {
	var pos int64 = (length(text) - 1)
	for pos >= 0 {
		if at(text, pos) == "/" {
			return part(text, 0, pos)
		}
		pos = (pos - 1)
	}
	return ""
}

func add_segment(path string, segment string) string {
	if (segment == "") || (segment == ".") {
		return path
	}
	if segment == ".." {
		return parent_path(path)
	}
	return ((path + "/") + segment)
}

func solve(text string) string {
	var path string = ""
	var start int64 = 1
	var pos int64 = 1
	for pos <= length(text) {
		if (pos == length(text)) || (at(text, pos) == "/") {
			path = add_segment(path, part(text, start, pos))
			start = (pos + 1)
		}
		pos = (pos + 1)
	}
	if path == "" {
		return "/"
	}
	return path
}
