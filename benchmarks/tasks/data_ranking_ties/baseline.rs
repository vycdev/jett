#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub score: i64,
    pub penalty: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub selected: i64,
    pub rank_sum: i64,
    pub tie_groups: i64,
}

pub fn ahead(score: i64, penalty: i64, other_score: i64, other_penalty: i64) -> bool {
    return ((other_score > score) || ((other_score == score) && (other_penalty < penalty)));
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut selected: i64 = 0;
    let mut rank_sum: i64 = 0;
    let mut groups: i64 = 0;
    let mut index: i64 = 0;
    for row in rows {
        let mut rank: i64 = 1;
        let mut ties: i64 = 0;
        let mut first: i64 = index;
        let mut other_index: i64 = 0;
        for other in rows {
            if ahead(row.score, row.penalty, other.score, other.penalty) {
                rank = (rank + 1);
            }
            if ((row.score == other.score) && (row.penalty == other.penalty)) {
                ties = (ties + 1);
                if (other_index < first) {
                    first = other_index;
                }
            }
            other_index = (other_index + 1);
        }
        if (rank <= limit) {
            selected = (selected + 1);
            rank_sum = (rank_sum + rank);
        }
        if ((ties > 1) && (first == index)) {
            groups = (groups + 1);
        }
        index = (index + 1);
    }
    return Report {
        selected: selected,
        rank_sum: rank_sum,
        tie_groups: groups,
    };
}
