package benchmark

import (
	"strconv"
	"strings"
)

type ScoreError int

const (
	Malformed ScoreError = iota
	InvalidScore
	DuplicateName
)

type ScoreResult interface{ isScoreResult() }

type ParsedScores struct{ Scores map[string]int64 }
type ScoreFailure struct {
	Line  int64
	Error ScoreError
}

func (ParsedScores) isScoreResult() {}
func (ScoreFailure) isScoreResult() {}

func ParseScores(lines []string) ScoreResult {
	scores := make(map[string]int64)
	for lineIndex, line := range lines {
		parts := strings.Split(line, "=")
		if len(parts) != 2 || parts[0] == "" {
			return ScoreFailure{Line: int64(lineIndex), Error: Malformed}
		}
		name, rawScore := parts[0], parts[1]
		score, error := strconv.ParseInt(rawScore, 10, 64)
		if error != nil || strconv.FormatInt(score, 10) != rawScore {
			return ScoreFailure{Line: int64(lineIndex), Error: InvalidScore}
		}
		if _, exists := scores[name]; exists {
			return ScoreFailure{Line: int64(lineIndex), Error: DuplicateName}
		}
		scores[name] = score
	}
	return ParsedScores{Scores: scores}
}
