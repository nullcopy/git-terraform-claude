use anyhow::{Result, anyhow};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc, Weekday};
use colored::*;
use dialoguer::Confirm;
use git2::Oid;

use crate::common::{OutputFormatter, ProgressIndicator, commit_to_datetime};
use crate::duration_format;
use crate::error::TerraformError;
use crate::git_ops::GitOps;
use crate::timeline::Timeline;

pub struct FitOptions {
    pub skip_weekends: bool,
    pub skip_weekdays: bool,
}

pub fn execute_fit(
    target_range: &str,
    commits_range: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    skip_weekends: bool,
    skip_weekdays: bool,
    no_preview: bool,
    allow_non_monotonic: bool,
    timeline_width: usize,
    verbose: bool,
) -> Result<()> {
    let git = GitOps::new()?;
    let current_branch = git.get_current_branch()?;
    
    println!("{} {} {}", 
        "Fitting timeline on branch".cyan(),
        current_branch.green().bold(),
        format!("(target: {})", target_range).dimmed()
    );
    
    // Parse target range
    let (start_time, end_time) = parse_target_range(target_range)?;
    
    if verbose {
        println!("{} Start: {}, End: {}", 
            "Target range:".dimmed(),
            start_time.format("%Y-%m-%d %H:%M:%S"),
            end_time.format("%Y-%m-%d %H:%M:%S")
        );
    }
    
    // Get and filter commits
    let commits = git.get_commits_in_range(commits_range)?;
    let commits = git.filter_commits(commits, since, until, None)?;
    
    if commits.is_empty() {
        return Err(TerraformError::NoMatchingCommits.into());
    }
    
    if verbose {
        println!("{} {} commits to fit", 
            "Found".dimmed(),
            commits.len().to_string().yellow()
        );
    }
    
    // Calculate fitted timeline
    let options = FitOptions { skip_weekends, skip_weekdays };
    let changes = fit_commits(&commits, start_time, end_time, &options)?;
    
    if changes.is_empty() {
        println!("{}", "No changes needed - commits already fit the target range".yellow());
        return Ok(());
    }
    
    // Display preview
    if !no_preview {
        display_fit_preview(&git.repo, &commits, &changes, start_time, end_time, timeline_width)?;
        
        // Check for non-monotonic timeline
        if !git.check_monotonic(&changes) && !allow_non_monotonic {
            return Err(TerraformError::NonMonotonicTimeline.into());
        }
        
        // Confirm changes
        if !Confirm::new()
            .with_prompt("Apply fit transformation?")
            .default(false)
            .interact()? 
        {
            return Err(TerraformError::UserCancelled.into());
        }
    }
    
    // Apply changes
    let progress = ProgressIndicator::new(changes.len() as u64, "Fitting commits");
    git.apply_timestamp_changes(&changes, |_| {
        progress.inc();
    })?;
    progress.finish("Fit complete");
    
    println!("{} Fitted {} commits to target range", 
        "✓".green().bold(),
        changes.len()
    );
    
    Ok(())
}

fn parse_target_range(range_str: &str) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    // Support various range formats
    // Format: "YYYY-MM-DD..YYYY-MM-DD" or "YYYY-MM-DD HH:MM:SS..YYYY-MM-DD HH:MM:SS"
    let parts: Vec<&str> = range_str.split("..").collect();
    
    if parts.len() != 2 {
        return Err(anyhow!("Invalid range format. Use: YYYY-MM-DD..YYYY-MM-DD or with times"));
    }
    
    let start = parse_datetime(parts[0].trim())?;
    let end = parse_datetime(parts[1].trim())?;
    
    if end <= start {
        return Err(anyhow!("End time must be after start time"));
    }
    
    Ok((start, end))
}

fn parse_datetime(dt_str: &str) -> Result<DateTime<Utc>> {
    // Try parsing with time first
    if let Ok(dt) = NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    
    // Try parsing date only (defaults to midnight)
    if let Ok(date) = NaiveDate::parse_from_str(dt_str, "%Y-%m-%d") {
        if let Some(dt) = date.and_hms_opt(0, 0, 0) {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
    }
    
    // Also support relative dates like "7 days ago"
    if let Some(duration) = parse_relative_duration(dt_str) {
        return Ok(Utc::now() + duration);
    }
    
    Err(anyhow!("Invalid datetime format: {}. Use YYYY-MM-DD or YYYY-MM-DD HH:MM:SS", dt_str))
}

fn parse_relative_duration(date_str: &str) -> Option<Duration> {
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    
    if parts.len() >= 3 && parts[2] == "ago" {
        let num = parts[0].parse::<i64>().ok()?;
        let duration = match parts[1] {
            "second" | "seconds" => Duration::seconds(num),
            "minute" | "minutes" => Duration::minutes(num),
            "hour" | "hours" => Duration::hours(num),
            "day" | "days" => Duration::days(num),
            "week" | "weeks" => Duration::weeks(num),
            "month" | "months" => Duration::days(num * 30),
            "year" | "years" => Duration::days(num * 365),
            _ => return None,
        };
        Some(-duration)
    } else if parts.len() >= 3 && parts[2] == "from" && parts.get(3) == Some(&"now") {
        let num = parts[0].parse::<i64>().ok()?;
        match parts[1] {
            "second" | "seconds" => Some(Duration::seconds(num)),
            "minute" | "minutes" => Some(Duration::minutes(num)),
            "hour" | "hours" => Some(Duration::hours(num)),
            "day" | "days" => Some(Duration::days(num)),
            "week" | "weeks" => Some(Duration::weeks(num)),
            "month" | "months" => Some(Duration::days(num * 30)),
            "year" | "years" => Some(Duration::days(num * 365)),
            _ => None,
        }
    } else {
        None
    }
}

fn fit_commits(
    commits: &[git2::Commit],
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    options: &FitOptions,
) -> Result<Vec<(Oid, DateTime<Utc>, DateTime<Utc>)>> {
    if commits.is_empty() {
        return Ok(Vec::new());
    }
    
    // Sort commits by timestamp
    let mut sorted_commits: Vec<_> = commits.iter()
        .map(|c| (c.id(), commit_to_datetime(c)))
        .collect();
    sorted_commits.sort_by_key(|(_, time)| *time);
    
    // Generate available timestamps in the target range
    let available_timestamps = generate_timestamps(start_time, end_time, options, sorted_commits.len())?;
    
    if available_timestamps.len() < sorted_commits.len() {
        return Err(anyhow!(
            "Cannot fit {} commits into the target range with the given constraints. Only {} timestamps available.",
            sorted_commits.len(),
            available_timestamps.len()
        ));
    }
    
    // Map commits to new timestamps
    let mut changes = Vec::new();
    for (i, (oid, original_time)) in sorted_commits.iter().enumerate() {
        let new_time = available_timestamps[i];
        if new_time != *original_time {
            changes.push((*oid, *original_time, new_time));
        }
    }
    
    Ok(changes)
}

fn generate_timestamps(
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    options: &FitOptions,
    count: usize,
) -> Result<Vec<DateTime<Utc>>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    
    if count == 1 {
        // Single commit goes at the start
        return Ok(vec![start_time]);
    }
    
    // Generate all possible timestamps
    let mut all_timestamps = Vec::new();
    let mut current = start_time;
    
    while current <= end_time {
        let weekday = current.weekday();
        let is_weekend = weekday == Weekday::Sat || weekday == Weekday::Sun;
        
        // Check if this timestamp should be skipped
        let skip = (options.skip_weekends && is_weekend) || 
                   (options.skip_weekdays && !is_weekend);
        
        if !skip {
            all_timestamps.push(current);
        }
        
        // Move to next hour (we'll distribute commits across hours)
        current = current + Duration::hours(1);
    }
    
    if all_timestamps.is_empty() {
        return Err(anyhow!("No valid timestamps available in the target range with the given constraints"));
    }
    
    // If we have more timestamps than needed, distribute evenly
    if all_timestamps.len() >= count {
        let mut result = Vec::new();
        
        // Always include first timestamp
        result.push(all_timestamps[0]);
        
        if count > 1 {
            // Calculate step size for even distribution
            let step = (all_timestamps.len() - 1) as f64 / (count - 1) as f64;
            
            for i in 1..count {
                let index = if i == count - 1 {
                    // Last commit always goes at the end
                    all_timestamps.len() - 1
                } else {
                    (i as f64 * step).round() as usize
                };
                result.push(all_timestamps[index.min(all_timestamps.len() - 1)]);
            }
        }
        
        Ok(result)
    } else {
        // Not enough timestamps
        Ok(all_timestamps)
    }
}

fn display_fit_preview(
    repo: &git2::Repository,
    all_commits: &[git2::Commit],
    changes: &[(Oid, DateTime<Utc>, DateTime<Utc>)],
    target_start: DateTime<Utc>,
    target_end: DateTime<Utc>,
    width: usize,
) -> Result<()> {
    let formatter = OutputFormatter::new(width);
    let timeline = Timeline::new(width);
    
    // Build a map of changes for quick lookup
    let changes_map: std::collections::HashMap<Oid, DateTime<Utc>> = changes.iter()
        .map(|(oid, _, new_time)| (*oid, *new_time))
        .collect();
    
    // Build complete timeline with all commits (changed and unchanged)
    let mut all_changes = Vec::new();
    for commit in all_commits {
        let oid = commit.id();
        let old_time = commit_to_datetime(commit);
        let new_time = changes_map.get(&oid).copied().unwrap_or(old_time);
        all_changes.push((oid, old_time, new_time));
    }
    
    // Calculate statistics
    let all_old_times: Vec<_> = all_changes.iter().map(|(_, old, _)| *old).collect();
    let all_new_times: Vec<_> = all_changes.iter().map(|(_, _, new)| *new).collect();
    
    let old_min = all_old_times.iter().min().copied().unwrap_or_else(Utc::now);
    let old_max = all_old_times.iter().max().copied().unwrap_or_else(Utc::now);
    let new_min = all_new_times.iter().min().copied().unwrap_or_else(Utc::now);
    let new_max = all_new_times.iter().max().copied().unwrap_or_else(Utc::now);
    
    let original_duration = old_max.signed_duration_since(old_min);
    let new_duration = new_max.signed_duration_since(new_min);
    let target_duration = target_end.signed_duration_since(target_start);
    
    // Display header
    print!("{}", formatter.header("Fit Preview"));
    
    // Display statistics
    println!("{}", "Statistics:".yellow());
    println!("  {} {}", 
        "Original duration:".dimmed(),
        duration_format::human_readable(&original_duration)
    );
    println!("  {} {}", 
        "Target duration:".dimmed(),
        duration_format::human_readable(&target_duration)
    );
    println!("  {} {}", 
        "Actual new duration:".dimmed(),
        duration_format::human_readable(&new_duration)
    );
    println!("  {} {} to {}", 
        "Target range:".dimmed(),
        target_start.format("%Y-%m-%d %H:%M:%S"),
        target_end.format("%Y-%m-%d %H:%M:%S")
    );
    println!("  {} {} to {}", 
        "Actual range:".dimmed(),
        new_min.format("%Y-%m-%d %H:%M:%S"),
        new_max.format("%Y-%m-%d %H:%M:%S")
    );
    println!("  {} {} commits modified", 
        "Changes:".dimmed(),
        changes.len()
    );
    
    // Generate timeline visualization
    let visualization = timeline.render_changes(repo, &all_changes);
    print!("{}", visualization);
    
    Ok(())
}