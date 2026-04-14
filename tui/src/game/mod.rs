pub mod neversold;
pub mod fee_share;

pub trait OpenGame {
    type A;
    type Ctx;
    type B;
    fn evaluate(&self, a: &Self::A, c: &Self::Ctx) -> Self::B;
}

#[derive(Clone, Debug)]
pub struct AddrHistory {
    pub addr: String,
    pub first_received_ts: Option<i64>,
    pub sold_count: u32,
    pub current_balance: u64,
    pub all_time_received: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trit { Plus, Zero, Minus }

#[derive(Clone, Debug)]
pub struct DiagnosticInfo {
    pub trit: Trit,
    pub label: &'static str,
    pub reason: String,
    pub held_for_secs: i64,
    pub sold_count: u32,
}
