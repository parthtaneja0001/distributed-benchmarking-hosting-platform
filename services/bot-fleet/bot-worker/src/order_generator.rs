use serde::Serialize;
use std::time::SystemTime;

#[derive(Serialize, Clone)]
pub struct Order {
    pub order_id: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub side: String,
    pub price: f64,
    pub quantity: u32,
    pub timestamp: u128,            // microseconds since epoch
}

pub fn generate_order(trader_id: u32, order_counter: u64) -> Order {
    let now_micros = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros();

    let order_type = if order_counter % 3 == 0 { "market".to_string() } else { "limit".to_string() };
    let side = if order_counter % 2 == 0 { "buy".to_string() } else { "sell".to_string() };
    let price = 100.0 + (order_counter % 100) as f64;

    Order {
        order_id: format!("bot{}-{}", trader_id, order_counter),
        order_type,
        side,
        price,
        quantity: 1,
        timestamp: now_micros,
    }
}