#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub voter: i64,
    pub choice: i64,
    pub weight: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub yes_weight: i64,
    pub no_weight: i64,
    pub quorum_met: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn contribution(choice: i64, weight: i64, wanted: i64) -> i64 {
    if ((choice == wanted) && (weight > 0)) {
        return weight;
    }
    return 0;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut yes_votes: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut no_votes: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut yes: i64 = 0;
    let mut no: i64 = 0;
    for row in rows {
        let mut next_yes: i64 = contribution(row.choice, row.weight, 1);
        let mut next_no: i64 = contribution(row.choice, row.weight, 0);
        yes = ((yes + next_yes) - yes_votes.get(&row.voter).copied().unwrap_or(0));
        no = ((no + next_no) - no_votes.get(&row.voter).copied().unwrap_or(0));
        let mut updated_value_0: i64 = next_yes;
        yes_votes = put(yes_votes, row.voter, updated_value_0);
        let mut updated_value_1: i64 = next_no;
        no_votes = put(no_votes, row.voter, updated_value_1);
    }
    let mut met: i64 = 0;
    if ((yes >= limit) && (yes > no)) {
        met = 1;
    }
    return Report {
        yes_weight: yes,
        no_weight: no,
        quorum_met: met,
    };
}
