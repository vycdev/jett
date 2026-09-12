#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub principal: i64,
    pub allowed: i64,
    pub specificity: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub allowed: i64,
    pub denied: i64,
    pub defaulted: i64,
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut allowed: i64 = 0;
    let mut denied: i64 = 0;
    let mut defaulted: i64 = 0;
    let mut principal: i64 = 0;
    while (principal < limit) {
        let mut priority: i64 = (-1);
        let mut decision: i64 = 0;
        for row in rows {
            if ((row.principal == principal) && (row.specificity >= priority)) {
                priority = row.specificity;
                decision = row.allowed;
            }
        }
        if (priority == (-1)) {
            defaulted = (defaulted + 1);
        }
        allowed = (allowed + decision);
        denied = ((denied + 1) - decision);
        principal = (principal + 1);
    }
    return Report {
        allowed: allowed,
        denied: denied,
        defaulted: defaulted,
    };
}
