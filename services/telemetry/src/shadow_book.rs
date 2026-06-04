use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct ExpectedFill {
    pub order_id: String,
    pub resting_order_id: String,
    pub price: f64,
    pub quantity: u32,
}

#[derive(Debug, Clone)]
struct PendingFill {
    order_id: String,
    fill_price: f64,
    fill_quantity: u32,
}

pub struct ShadowBook {
    buy_levels: Vec<PriceLevel>,
    sell_levels: Vec<PriceLevel>,
    orders: HashMap<String, OrderInfo>,
    pub expected_fills: Vec<ExpectedFill>,
    pub matched_fills: Vec<ExpectedFill>,
    pub extra_fills: u64,
    // fills that arrived before their order
    pending_fills: HashMap<String, VecDeque<PendingFill>>,
}

#[derive(Clone)]
struct OrderInfo {
    price: f64,
    quantity: u32,
    side: String,
}

struct PriceLevel {
    price: f64,
    orders: Vec<String>,
}

impl ShadowBook {
    pub fn new() -> Self {
        ShadowBook {
            buy_levels: Vec::new(),
            sell_levels: Vec::new(),
            orders: HashMap::new(),
            expected_fills: Vec::new(),
            matched_fills: Vec::new(),
            extra_fills: 0,
            pending_fills: HashMap::new(),
        }
    }

    pub fn process_order(
        &mut self,
        order_id: String,
        side: String,
        order_type: String,
        price: f64,
        quantity: u32,
    ) -> Vec<ExpectedFill> {
        let mut fills = Vec::new();
        let mut remaining = quantity;

        if order_type == "cancel" {
            self.remove_order(&order_id);
            return fills;
        }

        if order_type == "limit" || order_type == "market" {
            let match_price = if order_type == "market" {
                None
            } else {
                Some(price)
            };

            if side == "buy" {
                while remaining > 0 && !self.sell_levels.is_empty() {
                    let best_sell = &self.sell_levels[0];
                    if let Some(limit_price) = match_price {
                        if best_sell.price > limit_price {
                            break;
                        }
                    }
                    if let Some(resting_id) = best_sell.orders.first().cloned() {
                        let resting_qty = self.orders[&resting_id].quantity;
                        let fill_qty = remaining.min(resting_qty);
                        let fill_price = best_sell.price;

                        fills.push(ExpectedFill {
                            order_id: order_id.clone(),
                            resting_order_id: resting_id.clone(),
                            price: fill_price,
                            quantity: fill_qty,
                        });
                        fills.push(ExpectedFill {
                            order_id: resting_id.clone(),
                            resting_order_id: order_id.clone(),
                            price: fill_price,
                            quantity: fill_qty,
                        });

                        remaining -= fill_qty;
                        if let Some(info) = self.orders.get_mut(&resting_id) {
                            info.quantity -= fill_qty;
                            if info.quantity == 0 {
                                self.remove_order_from_level(&resting_id, "sell");
                            }
                        }

                        // Match any pending fills for BOTH sides of this trade
                        self.match_pending_fills(&order_id);
                        self.match_pending_fills(&resting_id);
                    } else {
                        self.sell_levels.remove(0);
                    }
                }
            } else {
                while remaining > 0 && !self.buy_levels.is_empty() {
                    let best_buy = &self.buy_levels[0];
                    if let Some(limit_price) = match_price {
                        if best_buy.price < limit_price {
                            break;
                        }
                    }
                    if let Some(resting_id) = best_buy.orders.first().cloned() {
                        let resting_qty = self.orders[&resting_id].quantity;
                        let fill_qty = remaining.min(resting_qty);
                        let fill_price = best_buy.price;

                        // Expected fill for the incoming order
                        fills.push(ExpectedFill {
                            order_id: order_id.clone(),
                            resting_order_id: resting_id.clone(),
                            price: fill_price,
                            quantity: fill_qty,
                        });
                        // Expected fill for the resting order
                        fills.push(ExpectedFill {
                            order_id: resting_id.clone(),
                            resting_order_id: order_id.clone(),
                            price: fill_price,
                            quantity: fill_qty,
                        });

                        remaining -= fill_qty;
                        if let Some(info) = self.orders.get_mut(&resting_id) {
                            info.quantity -= fill_qty;
                            if info.quantity == 0 {
                                self.remove_order_from_level(&resting_id, "buy");
                            }
                        }
                    } else {
                        self.buy_levels.remove(0);
                    }
                }
            }

            if order_type == "limit" && remaining > 0 {
                self.add_order(order_id.clone(), side, price, remaining);
            }
        }

        // Store expected fills
        self.expected_fills.extend(fills.clone());

        // After generating fills, try to match any pending fills for this order
        self.match_pending_fills(&order_id);

        fills
    }

    fn match_pending_fills(&mut self, order_id: &str) {
        if let Some(pending_queue) = self.pending_fills.remove(order_id) {
            for pending in pending_queue {
                let mut matched = false;
                for (i, expected) in self.expected_fills.iter().enumerate() {
                    if expected.order_id == pending.order_id
                        && (expected.price - pending.fill_price).abs() < 1e-9
                        && expected.quantity == pending.fill_quantity
                    {
                        self.matched_fills.push(expected.clone());
                        self.expected_fills.remove(i);
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    self.extra_fills += 1;
                }
            }
        }
    }

    pub fn match_fill(&mut self, order_id: &str, fill_price: f64, fill_quantity: u32) -> bool {
        // First try to match against existing expected fills
        for (i, expected) in self.expected_fills.iter().enumerate() {
            if expected.order_id == order_id
                && (expected.price - fill_price).abs() < 1e-9
                && expected.quantity == fill_quantity
            {
                self.matched_fills.push(expected.clone());
                self.expected_fills.remove(i);
                return true;
            }
        }

        // If no match yet, store as pending
        self.pending_fills
            .entry(order_id.to_string())
            .or_insert(VecDeque::new())
            .push_back(PendingFill {
                order_id: order_id.to_string(),
                fill_price,
                fill_quantity,
            });
        false // not matched yet, but not counted as extra
    }

    pub fn correctness(&self) -> f64 {
        let matched = self.matched_fills.len() as f64;
        let unmatched = self.expected_fills.len() as f64;
        let extra = self.extra_fills as f64;
        let total = matched + unmatched + extra;
        if total == 0.0 {
            1.0
        } else {
            matched / total
        }
    }

    // --- internal helpers ---
    fn add_order(&mut self, id: String, side: String, price: f64, quantity: u32) {
        self.orders.insert(
            id.clone(),
            OrderInfo {
                price,
                quantity,
                side: side.clone(),
            },
        );

        let levels = if side == "buy" {
            &mut self.buy_levels
        } else {
            &mut self.sell_levels
        };
        let level = levels.iter_mut().find(|l| l.price == price);
        match level {
            Some(level) => level.orders.push(id),
            None => {
                let new_level = PriceLevel {
                    price,
                    orders: vec![id],
                };
                levels.push(new_level);
                if side == "buy" {
                    levels.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
                } else {
                    levels.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
                }
            }
        }
    }

    fn remove_order(&mut self, id: &str) {
        if let Some(info) = self.orders.remove(id) {
            self.remove_order_from_level(id, &info.side);
        }
    }

    fn remove_order_from_level(&mut self, id: &str, side: &str) {
        let levels = if side == "buy" {
            &mut self.buy_levels
        } else {
            &mut self.sell_levels
        };
        for i in 0..levels.len() {
            let level = &mut levels[i];
            if let Some(pos) = level.orders.iter().position(|oid| oid == id) {
                level.orders.remove(pos);
                if level.orders.is_empty() {
                    levels.remove(i);
                }
                break;
            }
        }
    }
}
