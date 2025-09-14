use chrono::Duration;

/// Formats a duration in compact notation suitable for parsing and technical display.
/// Examples: "+3d2h1m", "-2h30m", "+0s"
/// 
/// This format is:
/// - Machine-readable and parseable
/// - Compact for command-line display
/// - Includes sign prefix for clarity about direction
pub fn compact(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds().abs();
    let sign = if duration.num_seconds() < 0 { "-" } else { "+" };
    
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    let mut parts = Vec::new();
    
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{}s", seconds));
    }
    
    format!("{}{}", sign, parts.join(""))
}

/// Formats a duration in human-readable form for display to users.
/// Examples: "3 days, 2 hours", "2 hours, 30 minutes", "45 minutes"
/// 
/// This format is:
/// - Natural language friendly
/// - Easy to understand at a glance
/// - Automatically scales units (years, months, days, hours, minutes)
pub fn human_readable(duration: &Duration) -> String {
    let days = duration.num_days().abs();
    let hours = (duration.num_hours() % 24).abs();
    let minutes = (duration.num_minutes() % 60).abs();
    
    if days > 365 {
        format!("{} years, {} days", days / 365, days % 365)
    } else if days > 30 {
        format!("{} months, {} days", days / 30, days % 30)
    } else if days > 0 {
        format!("{} days, {} hours", days, hours)
    } else if hours > 0 {
        format!("{} hours, {} minutes", hours, minutes)
    } else {
        format!("{} minutes", minutes)
    }
}

/// Formats a duration with an explicit sign in human-readable form.
/// Examples: "+3 days, 2 hours", "-2 hours, 30 minutes"
/// 
/// Useful when the direction of change needs to be emphasized.
pub fn human_readable_with_sign(duration: &Duration) -> String {
    let sign = if duration.num_seconds() < 0 { "-" } else { "+" };
    format!("{}{}", sign, human_readable(duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compact_format() {
        assert_eq!(compact(&Duration::days(3)), "+3d");
        assert_eq!(compact(&Duration::hours(-2)), "-2h");
        assert_eq!(compact(&Duration::seconds(3661)), "+1h1m1s");
        assert_eq!(compact(&Duration::zero()), "+0s");
    }
    
    #[test]
    fn test_human_readable_format() {
        assert_eq!(human_readable(&Duration::days(3)), "3 days, 0 hours");
        assert_eq!(human_readable(&Duration::hours(25)), "1 days, 1 hours");
        assert_eq!(human_readable(&Duration::minutes(90)), "1 hours, 30 minutes");
        assert_eq!(human_readable(&Duration::minutes(45)), "45 minutes");
        assert_eq!(human_readable(&Duration::days(400)), "1 years, 35 days");
    }
    
    #[test]
    fn test_human_readable_with_sign() {
        assert_eq!(human_readable_with_sign(&Duration::days(3)), "+3 days, 0 hours");
        assert_eq!(human_readable_with_sign(&Duration::hours(-2)), "-2 hours, 0 minutes");
    }
}