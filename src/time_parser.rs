use anyhow::{Result, anyhow};
use chrono::Duration;
use regex::Regex;
use once_cell::sync::Lazy;

// Compile regex once at startup
static TIME_PARTS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d+)([smhdwMy])").expect("valid regex pattern")
});

pub fn parse_time_shift(input: &str) -> Result<Duration> {
    let mut total_duration = Duration::zero();
    let mut remaining = input;
    
    if remaining.is_empty() {
        return Err(anyhow!("Parse error: Empty time shift"));
    }
    
    let is_negative = remaining.starts_with('-');
    if remaining.starts_with('+') || remaining.starts_with('-') {
        remaining = &remaining[1..];
    }
    
    for cap in TIME_PARTS_RE.captures_iter(remaining) {
        let value: i64 = cap[1].parse().map_err(|_| 
            anyhow!("Parse error: Invalid number: {}", &cap[1]))?;
        
        let unit_duration = match &cap[2] {
            "s" => Duration::seconds(value),
            "m" => Duration::minutes(value),
            "h" => Duration::hours(value),
            "d" => Duration::days(value),
            "w" => Duration::weeks(value),
            "M" => Duration::days(value * 30), // Approximate month
            "y" => Duration::days(value * 365), // Approximate year
            _ => return Err(anyhow!(
                "Parse error: Invalid time unit: {}", &cap[2]
            )),
        };
        
        total_duration += unit_duration;
    }
    
    if total_duration == Duration::zero() {
        return Err(anyhow!("Parse error: Invalid time shift format"));
    }
    
    Ok(if is_negative { -total_duration } else { total_duration })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_time_shift() {
        assert_eq!(parse_time_shift("+3d").expect("valid shift"), Duration::days(3));
        assert_eq!(parse_time_shift("-2h").expect("valid shift"), Duration::hours(-2));
        assert_eq!(parse_time_shift("+1y6M").expect("valid shift"), 
                   Duration::days(365) + Duration::days(180));
        assert_eq!(parse_time_shift("-30m").expect("valid shift"), Duration::minutes(-30));
    }
    
    #[test]
    fn test_format_duration() {
        // Test that compact formatting works correctly via the duration_format module
        assert_eq!(crate::duration_format::compact(&Duration::days(3)), "+3d");
        assert_eq!(crate::duration_format::compact(&Duration::hours(-2)), "-2h");
        assert_eq!(crate::duration_format::compact(&Duration::seconds(3661)), "+1h1m1s");
    }
}