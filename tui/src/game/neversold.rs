use super::{AddrHistory, DiagnosticInfo, OpenGame, Trit};

pub struct NeversoldGame { pub now_ts: i64 }

impl OpenGame for NeversoldGame {
    type A = String;
    type Ctx = AddrHistory;
    type B = DiagnosticInfo;

    fn evaluate(&self, _addr: &String, c: &AddrHistory) -> DiagnosticInfo {
        let held = c.first_received_ts.map(|t| self.now_ts - t).unwrap_or(0);
        let (trit, label, reason) = match c.sold_count {
            0 => (Trit::Plus, "Neversold",
                  format!("never sold; balance {}", c.current_balance)),
            1 => (Trit::Zero, "AlmostNeversold",
                  format!("sold once; balance {}/{}", c.current_balance, c.all_time_received)),
            n => (Trit::Minus, "Sold",
                  format!("sold {} times", n)),
        };
        DiagnosticInfo { trit, label, reason, held_for_secs: held, sold_count: c.sold_count }
    }
}
