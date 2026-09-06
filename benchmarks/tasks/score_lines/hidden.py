from solution import ParsedScores, ScoreError, ScoreFailure, parse_scores


assert parse_scores([]) == ParsedScores({})
assert parse_scores(["ada=10", "bob=-2"]) == ParsedScores({"ada": 10, "bob": -2})
assert parse_scores(["ada=9223372036854775807", "bob=-9223372036854775808"]) == ParsedScores({"ada": 9223372036854775807, "bob": -9223372036854775808})
assert parse_scores(["ada=1", "ada=2"]) == ScoreFailure(1, ScoreError.DUPLICATE_NAME)
assert parse_scores(["missing"]) == ScoreFailure(0, ScoreError.MALFORMED)
assert parse_scores(["=3"]) == ScoreFailure(0, ScoreError.MALFORMED)
assert parse_scores(["a=1=2"]) == ScoreFailure(0, ScoreError.MALFORMED)
assert parse_scores(["ada=01"]) == ScoreFailure(0, ScoreError.INVALID_SCORE)
assert parse_scores(["ada=-0"]) == ScoreFailure(0, ScoreError.INVALID_SCORE)
assert parse_scores(["ada=9223372036854775808"]) == ScoreFailure(0, ScoreError.INVALID_SCORE)
assert parse_scores(["ada=1", "ada=bad"]) == ScoreFailure(1, ScoreError.INVALID_SCORE)
