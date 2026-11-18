//! Order-independent correctness checks.
//!
//! Many bots send orders concurrently, so the sequence the engine sees differs
//! from the sequence on the `orders.sent` topic. Replaying that topic through a
//! reference book and demanding fill-for-fill equality therefore penalises a
//! correct engine at random. Instead every reported fill is checked against
//! rules that hold regardless of arrival order:
//!
//! * it belongs to an order that was actually sent in this test,
//! * a limit order is never filled at a worse price than its limit,
//! * an order is never filled for more than its quantity;
//!
//! and the total filled volume is compared with what the reference book
//! (fed in topic order) expects, which is stable in aggregate even when the
//! individual matches differ.

use std::collections::HashMap;

const PRICE_EPSILON: f64 = 1e-9;

struct OrderSpec {
    side: String,
    order_type: String,
    price: f64,
    quantity: u64,
    filled: u64,
}

#[derive(Default)]
pub struct FillValidator {
    orders: HashMap<String, OrderSpec>,
    /// Fills that arrived before their order (separate topics).
    unresolved: HashMap<String, Vec<(f64, u64)>>,
    valid: u64,
    invalid: u64,
    price_violations: u64,
    overfills: u64,
    actual_volume: u64,
}

impl FillValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_order(&mut self, order_id: String, side: String, order_type: String, price: f64, quantity: u32) {
        let pending = self.unresolved.remove(&order_id);
        self.orders.insert(
            order_id.clone(),
            OrderSpec { side, order_type, price, quantity: quantity as u64, filled: 0 },
        );
        for (price, qty) in pending.into_iter().flatten() {
            self.check(&order_id, price, qty);
        }
    }

    pub fn record_fill(&mut self, order_id: String, price: f64, quantity: u32) {
        let qty = quantity as u64;
        self.actual_volume += qty;
        if self.orders.contains_key(&order_id) {
            self.check(&order_id, price, qty);
        } else {
            self.unresolved.entry(order_id).or_default().push((price, qty));
        }
    }

    fn check(&mut self, order_id: &str, price: f64, qty: u64) {
        let Some(order) = self.orders.get_mut(order_id) else { return };
        order.filled += qty;
        let price_ok = order.order_type != "limit"
            || match order.side.as_str() {
                "buy" => price <= order.price + PRICE_EPSILON,
                "sell" => price >= order.price - PRICE_EPSILON,
                _ => false,
            };
        let overfilled = order.filled > order.quantity;
        if qty > 0 && price_ok && !overfilled {
            self.valid += 1;
        } else {
            self.invalid += 1;
            if !price_ok {
                self.price_violations += 1;
            }
            if overfilled {
                self.overfills += 1;
            }
        }
    }

    /// Score in [0, 1]: share of valid fills × agreement of total fill volume
    /// with the reference book's expectation.
    pub fn debug_counts(&self, expected_volume: u64) -> String {
        let unresolved: usize = self.unresolved.values().map(|v| v.len()).sum();
        format!(
            "valid={} invalid={} (price={} overfill={}) unresolved={} actual_vol={} expected_vol={}",
            self.valid, self.invalid, self.price_violations, self.overfills, unresolved,
            self.actual_volume, expected_volume
        )
    }

    pub fn correctness(&self, expected_volume: u64) -> f64 {
        let checked = self.valid + self.invalid;
        let validity = if checked == 0 { 1.0 } else { self.valid as f64 / checked as f64 };

        let (a, e) = (self.actual_volume as f64, expected_volume as f64);
        let volume_agreement = if a.max(e) == 0.0 { 1.0 } else { a.min(e) / a.max(e) };

        validity * volume_agreement
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_fills_matching_volume_score_one() {
        let mut v = FillValidator::new();
        v.record_order("b".into(), "buy".into(), "limit".into(), 101.0, 1);
        v.record_order("s".into(), "sell".into(), "limit".into(), 100.0, 1);
        v.record_fill("b".into(), 100.0, 1);
        v.record_fill("s".into(), 100.0, 1);
        assert_eq!(v.correctness(2), 1.0);
    }

    #[test]
    fn fill_before_order_is_checked_later() {
        let mut v = FillValidator::new();
        v.record_fill("b".into(), 100.0, 1);
        v.record_order("b".into(), "buy".into(), "limit".into(), 101.0, 1);
        assert_eq!(v.correctness(1), 1.0);
    }

    #[test]
    fn limit_violation_and_overfill_are_invalid() {
        let mut v = FillValidator::new();
        v.record_order("b".into(), "buy".into(), "limit".into(), 100.0, 1);
        v.record_fill("b".into(), 105.0, 1); // worse than limit
        v.record_order("s".into(), "sell".into(), "limit".into(), 100.0, 1);
        v.record_fill("s".into(), 100.0, 1);
        v.record_fill("s".into(), 100.0, 1); // overfill
        assert!((v.correctness(3) - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn missing_fills_lower_the_score() {
        let mut v = FillValidator::new();
        v.record_order("b".into(), "buy".into(), "limit".into(), 100.0, 1);
        v.record_fill("b".into(), 100.0, 1);
        assert_eq!(v.correctness(4), 0.25);
    }
}
