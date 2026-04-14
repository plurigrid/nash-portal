use super::{AddrHistory, OpenGame};

pub struct FeeShareGame {
    pub now_ts: i64,
    pub total_tier_bps: u32,
}

impl OpenGame for FeeShareGame {
    type A = String;
    type Ctx = AddrHistory;
    type B = u32;

    fn evaluate(&self, _addr: &String, c: &AddrHistory) -> u32 {
        if c.sold_count > 1 { return 0; }
        let held = c.first_received_ts.map(|t| self.now_ts - t).unwrap_or(0).max(0) as u64;
        let held_mult = (held / 86_400).min(90) as u32;
        let neversold_mult: u32 = if c.sold_count == 0 { 3 } else { 1 };
        let bal_weight = ((c.current_balance as f64).sqrt() as u32).min(10_000);
        let raw = held_mult * neversold_mult * bal_weight;
        raw.min(self.total_tier_bps)
    }
}
