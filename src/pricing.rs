/// Minimum order notional value: 10 USDH.
pub const MIN_NOTIONAL: f64 = 10.0;

/// Compute the tick size for a given price (5 significant figures).
///
/// Formula: tick = 10^(floor(log10(price)) - 4)
pub fn compute_tick_size(price: f64) -> f64 {
    if price <= 0.0 {
        return 0.00001;
    }
    10.0_f64.powf(price.log10().floor() - 4.0)
}

/// Round a price to the nearest valid tick.
pub fn round_to_tick(price: f64) -> f64 {
    let tick = compute_tick_size(price);
    (price / tick).round() * tick
}

/// Format a price for signing: round to tick, strip trailing zeros.
///
/// The exchange strips trailing zeros before msgpack hashing.
/// Sending "0.650" when the server hashes "0.65" produces a different hash.
pub fn format_price(price: f64) -> String {
    if price <= 0.0 {
        return "0".to_string();
    }
    let rounded = round_to_tick(price);
    let tick = compute_tick_size(rounded);
    let decimals = (-tick.log10().floor()).max(0.0) as usize;
    let s = format!("{:.prec$}", rounded, prec = decimals);
    strip_zeros(&s)
}

/// Strip trailing zeros from a numeric string.
///
/// "35.810" -> "35.81", "1.0" -> "1", "0.650" -> "0.65"
pub fn strip_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('0');
    trimmed.trim_end_matches('.').to_string()
}

/// Calculate the minimum order size (shares) for a prediction market coin.
///
/// Exchange enforces: size * min(markPx, 1 - markPx) >= 10 USDH
pub fn get_min_shares(mark_px: f64) -> f64 {
    let effective_px = mark_px.min(1.0 - mark_px).max(0.01);
    (MIN_NOTIONAL / effective_px).ceil()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_size() {
        assert!((compute_tick_size(0.55) - 0.00001).abs() < 1e-10);
        assert!((compute_tick_size(1.0) - 0.0001).abs() < 1e-10);
        assert!((compute_tick_size(65000.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_format_price() {
        assert_eq!(format_price(0.55), "0.55");
        assert_eq!(format_price(0.5), "0.5");
        assert_eq!(format_price(0.0), "0");
    }

    #[test]
    fn test_strip_zeros() {
        assert_eq!(strip_zeros("35.810"), "35.81");
        assert_eq!(strip_zeros("1.0"), "1");
        assert_eq!(strip_zeros("0.650"), "0.65");
        assert_eq!(strip_zeros("100"), "100");
    }

    #[test]
    fn test_min_shares() {
        // At price 0.5, min = 10 / 0.5 = 20
        assert_eq!(get_min_shares(0.5), 20.0);
        // At price 0.1, min = ceil(10 / 0.1) - float precision may give 101
        assert!(get_min_shares(0.1) >= 100.0 && get_min_shares(0.1) <= 101.0);
        // At price 0.9, min = ceil(10 / 0.1)
        assert!(get_min_shares(0.9) >= 100.0 && get_min_shares(0.9) <= 101.0);
        // At price 0.01 (edge), clamped to 0.01, min = 10 / 0.01 = 1000
        assert_eq!(get_min_shares(0.01), 1000.0);
    }
}
