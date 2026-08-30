package benchmark

import (
	"reflect"
	"testing"
)

func TestScoreLines(t *testing.T) {
	tests := []struct {
		lines []string
		want  ScoreResult
	}{
		{[]string{}, ParsedScores{Scores: map[string]int64{}}},
		{[]string{"ada=10", "bob=-2"}, ParsedScores{Scores: map[string]int64{"ada": 10, "bob": -2}}},
		{[]string{"ada=9223372036854775807", "bob=-9223372036854775808"}, ParsedScores{Scores: map[string]int64{"ada": 9223372036854775807, "bob": -9223372036854775808}}},
		{[]string{"ada=1", "ada=2"}, ScoreFailure{Line: 1, Error: DuplicateName}},
		{[]string{"missing"}, ScoreFailure{Line: 0, Error: Malformed}},
		{[]string{"=3"}, ScoreFailure{Line: 0, Error: Malformed}},
		{[]string{"a=1=2"}, ScoreFailure{Line: 0, Error: Malformed}},
		{[]string{"ada=01"}, ScoreFailure{Line: 0, Error: InvalidScore}},
		{[]string{"ada=-0"}, ScoreFailure{Line: 0, Error: InvalidScore}},
		{[]string{"ada=9223372036854775808"}, ScoreFailure{Line: 0, Error: InvalidScore}},
		{[]string{"ada=1", "ada=bad"}, ScoreFailure{Line: 1, Error: InvalidScore}},
	}
	for _, test := range tests {
		if got := ParseScores(test.lines); !reflect.DeepEqual(got, test.want) {
			t.Fatalf("got %#v, want %#v", got, test.want)
		}
	}
}
