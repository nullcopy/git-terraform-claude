use chrono::{DateTime, Utc, Duration, Datelike, Timelike, TimeZone};

type MarkerPositions = (Vec<(usize, String)>, Vec<(usize, String)>);

/// Generates dual-scale markers (minor and major) based on time range
pub fn generate_dual_scale_markers(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    width: usize,
) -> MarkerPositions {
    let duration = end.signed_duration_since(start);
    let days = duration.num_days();
    
    match days {
        0 => {
            // Same day: 15-minute intervals and hours
            (
                generate_time_markers(start, end, width, TimeScale::Minutes(15)),
                generate_time_markers(start, end, width, TimeScale::Hours(6)),
            )
        }
        1..=2 => {
            // 1-2 days: hours and days
            (
                generate_time_markers(start, end, width, TimeScale::Hours(6)),
                generate_time_markers(start, end, width, TimeScale::Days(1)),
            )
        }
        3..=7 => {
            // Week: days and endpoints
            (
                generate_time_markers(start, end, width, TimeScale::Days(1)),
                vec![
                    (0, start.format("%b %d").to_string()),
                    (width - 1, end.format("%b %d").to_string()),
                ],
            )
        }
        8..=30 => {
            // Month: weeks
            (
                generate_time_markers(start, end, width, TimeScale::Weeks(1)),
                vec![(0, start.format("%b %Y").to_string())],
            )
        }
        31..=90 => {
            // Quarter: bi-weekly and months
            (
                generate_time_markers(start, end, width, TimeScale::BiWeekly),
                generate_time_markers(start, end, width, TimeScale::Months),
            )
        }
        91..=365 => {
            // Year: months and quarters
            (
                generate_time_markers(start, end, width, TimeScale::Months),
                generate_time_markers(start, end, width, TimeScale::Quarters),
            )
        }
        366..=730 => {
            // 1-2 years: months and years
            (
                generate_time_markers(start, end, width, TimeScale::Months),
                generate_time_markers(start, end, width, TimeScale::Years),
            )
        }
        _ => {
            // Multiple years: quarters and years
            (
                generate_time_markers(start, end, width, TimeScale::Quarters),
                generate_time_markers(start, end, width, TimeScale::Years),
            )
        }
    }
}

/// Formats date labels with proper spacing
pub fn format_date_labels(markers: &[(usize, String)], width: usize) -> String {
    let mut label_line = vec![' '; width];
    
    // Place labels, avoiding overlaps
    let mut last_end = 0;
    for (pos, label) in markers {
        let start_pos = *pos;
        if start_pos >= last_end && start_pos + label.len() <= width {
            for (i, ch) in label.chars().enumerate() {
                if start_pos + i < width {
                    label_line[start_pos + i] = ch;
                }
            }
            last_end = start_pos + label.len() + 1; // +1 for spacing
        }
    }
    
    let label_str: String = label_line.iter().collect();
    format!("{}\n", label_str)
}

fn generate_time_markers(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    width: usize,
    scale: TimeScale,
) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    let total_duration = end.signed_duration_since(start);
    
    let mut current = round_to_scale(start, &scale);
    if current < start {
        current = advance_by_scale(current, &scale);
    }
    
    // Special handling for months to include start/end
    if matches!(scale, TimeScale::Months) {
        let start_pos = time_to_position(start, start, total_duration, width);
        markers.push((start_pos.min(5), start.format("%b").to_string()));
    }
    
    while current <= end {
        let pos = time_to_position(current, start, total_duration, width);
        let label = format_label(current, &scale);
        markers.push((pos, label));
        current = advance_by_scale(current, &scale);
    }
    
    // Add end month if needed
    if matches!(scale, TimeScale::Months) && !markers.is_empty() {
        let last_label = &markers.last().unwrap().1;
        let end_month = end.format("%b").to_string();
        if *last_label != end_month && end.day() > 1 {
            let end_pos = time_to_position(end, start, total_duration, width);
            markers.push((end_pos.max(width - 5), end_month));
        }
    }
    
    markers
}

fn round_to_scale(time: DateTime<Utc>, scale: &TimeScale) -> DateTime<Utc> {
    match scale {
        TimeScale::Minutes(interval) => {
            let next_minute = ((time.minute() / interval) + 1) * interval;
            if next_minute >= 60 {
                time + Duration::hours(1) - Duration::minutes(time.minute() as i64)
            } else {
                time + Duration::minutes((next_minute - time.minute()) as i64)
                    - Duration::seconds(time.second() as i64)
            }
        }
        TimeScale::Hours(_) => {
            Utc.with_ymd_and_hms(time.year(), time.month(), time.day(), time.hour(), 0, 0)
                .single()
                .expect("valid hour timestamp")
        }
        TimeScale::Days(_) => {
            Utc.with_ymd_and_hms(time.year(), time.month(), time.day(), 0, 0, 0)
                .single()
                .expect("valid day timestamp")
        }
        TimeScale::Weeks(_) => {
            let days_from_monday = time.weekday().num_days_from_monday() as i64;
            let week_start = time - Duration::days(days_from_monday);
            Utc.with_ymd_and_hms(
                week_start.year(),
                week_start.month(),
                week_start.day(),
                0, 0, 0,
            )
            .single()
            .expect("valid week timestamp")
        }
        TimeScale::BiWeekly => {
            if time.day() <= 15 {
                Utc.with_ymd_and_hms(time.year(), time.month(), 1, 0, 0, 0)
            } else {
                Utc.with_ymd_and_hms(time.year(), time.month(), 15, 0, 0, 0)
            }
            .single()
            .expect("valid biweekly timestamp")
        }
        TimeScale::Months => {
            Utc.with_ymd_and_hms(time.year(), time.month(), 1, 0, 0, 0)
                .single()
                .expect("valid month timestamp")
        }
        TimeScale::Quarters => {
            let quarter = ((time.month() - 1) / 3) + 1;
            let month = (quarter - 1) * 3 + 1;
            Utc.with_ymd_and_hms(time.year(), month, 1, 0, 0, 0)
                .single()
                .expect("valid quarter timestamp")
        }
        TimeScale::Years => {
            Utc.with_ymd_and_hms(time.year() + 1, 1, 1, 0, 0, 0)
                .single()
                .expect("valid year timestamp")
        }
    }
}

fn advance_by_scale(time: DateTime<Utc>, scale: &TimeScale) -> DateTime<Utc> {
    match scale {
        TimeScale::Minutes(interval) => time + Duration::minutes(*interval as i64),
        TimeScale::Hours(interval) => time + Duration::hours(*interval),
        TimeScale::Days(interval) => time + Duration::days(*interval),
        TimeScale::Weeks(interval) => time + Duration::weeks(*interval),
        TimeScale::BiWeekly => {
            if time.day() == 1 {
                Utc.with_ymd_and_hms(time.year(), time.month(), 15, 0, 0, 0)
                    .single()
                    .expect("valid biweekly timestamp")
            } else {
                next_month_start(time)
            }
        }
        TimeScale::Months => next_month_start(time),
        TimeScale::Quarters => {
            let quarter = ((time.month() - 1) / 3) + 1;
            if quarter < 4 {
                let month = quarter * 3 + 1;
                Utc.with_ymd_and_hms(time.year(), month, 1, 0, 0, 0)
                    .single()
                    .expect("valid quarter timestamp")
            } else {
                Utc.with_ymd_and_hms(time.year() + 1, 1, 1, 0, 0, 0)
                    .single()
                    .expect("valid quarter timestamp")
            }
        }
        TimeScale::Years => {
            Utc.with_ymd_and_hms(time.year() + 1, 1, 1, 0, 0, 0)
                .single()
                .expect("valid year timestamp")
        }
    }
}

fn format_label(time: DateTime<Utc>, scale: &TimeScale) -> String {
    match scale {
        TimeScale::Minutes(_) | TimeScale::Hours(_) => time.format("%H:%M").to_string(),
        TimeScale::Days(_) | TimeScale::Weeks(_) => time.format("%m/%d").to_string(),
        TimeScale::BiWeekly => time.format("%b %d").to_string(),
        TimeScale::Months => time.format("%b").to_string(),
        TimeScale::Quarters => {
            let quarter = ((time.month() - 1) / 3) + 1;
            format!("Q{}", quarter)
        }
        TimeScale::Years => time.format("%Y").to_string(),
    }
}

pub fn time_to_position(
    time: DateTime<Utc>,
    min_time: DateTime<Utc>,
    total_duration: Duration,
    width: usize,
) -> usize {
    if total_duration.num_seconds() == 0 {
        return width / 2;
    }
    
    let offset = time.signed_duration_since(min_time);
    let ratio = offset.num_seconds() as f64 / total_duration.num_seconds() as f64;
    let position = (ratio * width as f64) as usize;
    
    position.min(width - 1)
}

fn next_month_start(current: DateTime<Utc>) -> DateTime<Utc> {
    if current.month() == 12 {
        Utc.with_ymd_and_hms(current.year() + 1, 1, 1, 0, 0, 0)
    } else {
        Utc.with_ymd_and_hms(current.year(), current.month() + 1, 1, 0, 0, 0)
    }
    .single()
    .expect("valid month timestamp")
}

enum TimeScale {
    Minutes(u32),
    Hours(i64),
    Days(i64),
    Weeks(i64),
    BiWeekly,
    Months,
    Quarters,
    Years,
}