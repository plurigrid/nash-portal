use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefectorTier {
    pub wallet_address: String,
    pub tier: String,
    pub confidence_score: f64,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeversoldWallet {
    pub wallet_address: String,
    pub tier: String,
    pub verified: bool,
    pub exclude_lp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnLockPool {
    pub pool_type: String, // "hybrid_50_50"
    pub burn_percentage: u8,
    pub lock_percentage: u8,
    pub ttl_days: u16,
    pub gf3_conserved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResults {
    pub version: String,
    pub tier_s_defectors: Vec<DefectorTier>,
    pub tier_a_defectors: Vec<DefectorTier>,
    pub neversold_wallets: Vec<NeversoldWallet>,
    pub neversold_top20: Vec<NeversoldWallet>,
    pub burn_lock_pool: BurnLockPool,
    pub scan_timestamp: DateTime<Utc>,
}

impl ScanResults {
    pub fn new() -> Self {
        Self {
            version: "0.1.0".to_string(),
            tier_s_defectors: Vec::new(),
            tier_a_defectors: Vec::new(),
            neversold_wallets: Vec::new(),
            neversold_top20: Vec::new(),
            burn_lock_pool: BurnLockPool {
                pool_type: "hybrid_50_50".to_string(),
                burn_percentage: 50,
                lock_percentage: 50,
                ttl_days: 180,
                gf3_conserved: true,
            },
            scan_timestamp: Utc::now(),
        }
    }

    pub fn add_tier_s_defector(&mut self, defector: DefectorTier) {
        self.tier_s_defectors.push(defector);
    }

    pub fn add_tier_a_defector(&mut self, defector: DefectorTier) {
        self.tier_a_defectors.push(defector);
    }

    pub fn add_neversold_wallet(&mut self, wallet: NeversoldWallet) {
        self.neversold_wallets.push(wallet);
    }

    pub fn add_neversold_top20(&mut self, wallet: NeversoldWallet) {
        self.neversold_top20.push(wallet);
    }

    pub fn generate_artifacts(&self) -> HashMap<String, String> {
        let mut artifacts = HashMap::new();
        
        // Generate notice.md
        let notice = format!(
            "# Defector Scan Results v{}\n\n## Summary\n- {} Tier S defectors identified\n- {} Tier A defectors identified\n- {} Neversold wallets verified (excl LP)\n\n## Pool Configuration\n- Type: {}\n- TTL: {} days\n- GF(3)-conserved: {}\n\nScan completed: {}\n",
            self.version,
            self.tier_s_defectors.len(),
            self.tier_a_defectors.len(),
            self.neversold_wallets.len(),
            self.burn_lock_pool.pool_type,
            self.burn_lock_pool.ttl_days,
            self.burn_lock_pool.gf3_conserved,
            self.scan_timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        );
        artifacts.insert("notice.md".to_string(), notice);
        
        // Generate mechanism.md
        let mechanism = format!(
            "# Burn/Lock Mechanism\n\n## Configuration\n- Burn: {}%\n- Lock: {}%\n- TTL: {} days\n- GF(3) Conservation: {}\n\n## Implementation\nHybrid 50/50 burn/lock pool with 180-day time-to-live and Galois Field (3) conservation properties.\n",
            self.burn_lock_pool.burn_percentage,
            self.burn_lock_pool.lock_percentage,
            self.burn_lock_pool.ttl_days,
            self.burn_lock_pool.gf3_conserved
        );
        artifacts.insert("mechanism.md".to_string(), mechanism);
        
        artifacts
    }

    pub fn to_edn_tier_s(&self) -> String {
        let mut edn = "[".to_string();
        for (i, defector) in self.tier_s_defectors.iter().enumerate() {
            if i > 0 { edn.push_str(" "); }
            edn.push_str(&format!(
                "{{:wallet \"{}\" :tier \"{}\" :confidence {} :last-activity \"{}\"}}",
                defector.wallet_address,
                defector.tier,
                defector.confidence_score,
                defector.last_activity.format("%Y-%m-%d")
            ));
        }
        edn.push(']');
        edn
    }

    pub fn to_edn_neversold_tier(&self) -> String {
        let mut edn = "[".to_string();
        for (i, wallet) in self.neversold_wallets.iter().enumerate() {
            if i > 0 { edn.push_str(" "); }
            edn.push_str(&format!(
                "{{:wallet \"{}\" :tier \"{}\" :verified {} :exclude-lp {}}}",
                wallet.wallet_address,
                wallet.tier,
                wallet.verified,
                wallet.exclude_lp
            ));
        }
        edn.push(']');
        edn
    }

    pub fn to_edn_neversold_top20(&self) -> String {
        let mut edn = "[".to_string();
        for (i, wallet) in self.neversold_top20.iter().enumerate() {
            if i > 0 { edn.push_str(" "); }
            edn.push_str(&format!(
                "{{:wallet \"{}\" :tier \"{}\" :verified {} :exclude-lp {}}}",
                wallet.wallet_address,
                wallet.tier,
                wallet.verified,
                wallet.exclude_lp
            ));
        }
        edn.push(']');
        edn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_results_creation() {
        let results = ScanResults::new();
        assert_eq!(results.version, "0.1.0");
        assert_eq!(results.burn_lock_pool.burn_percentage, 50);
        assert_eq!(results.burn_lock_pool.lock_percentage, 50);
        assert_eq!(results.burn_lock_pool.ttl_days, 180);
        assert!(results.burn_lock_pool.gf3_conserved);
    }
}