#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub stock: i64,
    pub daily_demand: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub restock_units: i64,
    pub at_risk: i64,
    pub worst_shortfall: i64,
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut units: i64 = 0;
    let mut risk: i64 = 0;
    let mut worst: i64 = 0;
    for row in rows {
        let mut shortage: i64 = ((row.daily_demand * limit) - row.stock);
        if (shortage > 0) {
            units = (units + shortage);
            risk = (risk + 1);
            if (shortage > worst) {
                worst = shortage;
            }
        }
    }
    return Report {
        restock_units: units,
        at_risk: risk,
        worst_shortfall: worst,
    };
}
