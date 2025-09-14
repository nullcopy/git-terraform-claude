use anyhow::Result;
use crate::common::{GitContext, CommitFilterConfig, OutputFormatter, commit_to_datetime};
use crate::error::TerraformError;
use crate::timeline::Timeline;
use chrono::{DateTime, Utc, Datelike, Timelike};
use colored::*;

pub fn execute_analyze(
    range: Option<&str>,
    branch: Option<&str>,
    compare: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    timeline_width: usize,
    verbose: bool,
) -> Result<()> {
    let git_context = GitContext::new()?;
    
    // Determine which branch to analyze
    let target_branch = if let Some(b) = branch {
        println!("Analyzing branch: {}", b.green());
        b.to_string()
    } else {
        println!("Analyzing current branch: {}", git_context.current_branch.green());
        git_context.current_branch.clone()
    };
    
    // Get commits for analysis
    let effective_range = if branch.is_some() {
        // Analyzing a different branch without checking out
        Some(format!("{}^..{}", target_branch, target_branch))
    } else {
        range.map(|r| r.to_string())
    };
    
    let filter_config = CommitFilterConfig::new(
        effective_range.as_deref(),
        since,
        until
    );
    
    let filtered = match git_context.get_filtered_commits(&filter_config, verbose) {
        Ok(commits) => commits,
        Err(e) if e.downcast_ref::<TerraformError>()
            .map_or(false, |ce| matches!(ce, TerraformError::NoMatchingCommits)) => {
            println!("No commits match the specified criteria");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    
    // Convert to analysis format
    let mut analysis_data: Vec<(git2::Oid, DateTime<Utc>, String)> = Vec::new();
    
    for commit in &filtered {
        let time = commit_to_datetime(commit);
        let author = commit.author().name().unwrap_or("Unknown").to_string();
        analysis_data.push((commit.id(), time, author));
    }
    
    // Sort by time
    analysis_data.sort_by_key(|(_, time, _)| *time);
    
    // Create timeline visualization
    let timeline = Timeline::new(timeline_width);
    let output = timeline.render_analysis(&analysis_data);
    
    println!("{}", output);
    
    // Compare with another branch if requested
    if let Some(other_branch) = compare {
        compare_branches(&git_context, &target_branch, other_branch, timeline_width, verbose)?;
    }
    
    // Additional statistics if verbose
    if verbose {
        print_detailed_statistics(&analysis_data, timeline_width);
    }
    
    Ok(())
}

fn compare_branches(
    git_context: &GitContext,
    current: &str,
    other: &str,
    width: usize,
    verbose: bool,
) -> Result<()> {
    let formatter = OutputFormatter::new(width);
    print!("{}", formatter.header(&format!("COMPARING {} WITH {}", current, other)));
    
    // Get commits unique to current branch
    let unique_current_range = format!("{}..{}", other, current);
    let unique_current = git_context.git_ops.get_commits_in_range(Some(&unique_current_range))?;
    
    // Get commits unique to other branch
    let unique_other_range = format!("{}..{}", current, other);
    let unique_other = git_context.git_ops.get_commits_in_range(Some(&unique_other_range))?;
    
    println!("Commits unique to {}: {}", current.green(), unique_current.len());
    println!("Commits unique to {}: {}", other.yellow(), unique_other.len());
    
    if verbose && !unique_current.is_empty() {
        println!("\nRecent unique commits in {}:", current);
        for commit in unique_current.iter().take(5) {
            let time = commit_to_datetime(commit);
            println!(
                "  {} {} {}",
                format!("{:.8}", commit.id()).cyan(),
                time.format("%Y-%m-%d"),
                commit.summary().unwrap_or("")
            );
        }
    }
    
    if verbose && !unique_other.is_empty() {
        println!("\nRecent unique commits in {}:", other);
        for commit in unique_other.iter().take(5) {
            let time = commit_to_datetime(commit);
            println!(
                "  {} {} {}",
                format!("{:.8}", commit.id()).yellow(),
                time.format("%Y-%m-%d"),
                commit.summary().unwrap_or("")
            );
        }
    }
    
    // Find divergence point
    if let Ok(merge_base) = git_context.git_ops.repo.merge_base(
        git_context.git_ops.repo.revparse_single(current)?.id(),
        git_context.git_ops.repo.revparse_single(other)?.id(),
    ) {
        let divergence_commit = git_context.git_ops.repo.find_commit(merge_base)?;
        let divergence_time = commit_to_datetime(&divergence_commit);
        
        println!("\nBranches diverged at:");
        println!(
            "  {} {} {}",
            format!("{:.8}", merge_base).magenta(),
            divergence_time.format("%Y-%m-%d %H:%M"),
            divergence_commit.summary().unwrap_or("")
        );
    }
    
    print!("{}", formatter.double_separator());
    
    Ok(())
}

fn print_detailed_statistics(commits: &[(git2::Oid, DateTime<Utc>, String)], width: usize) {
    let formatter = OutputFormatter::new(width);
    print!("{}", formatter.header("DETAILED STATISTICS"));
    
    // Activity by day of week
    let mut weekday_counts = [0; 7];
    for (_, time, _) in commits {
        weekday_counts[time.weekday().num_days_from_sunday() as usize] += 1;
    }
    
    println!("\nCommits by day of week:");
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    for (i, day) in days.iter().enumerate() {
        let count = weekday_counts[i];
        let bar_length = (count * 40 / commits.len().max(1)).min(40);
        let bar = "█".repeat(bar_length);
        println!("  {} {:3} {}", day, count, bar.cyan());
    }
    
    // Activity by hour
    let mut hour_counts = [0; 24];
    for (_, time, _) in commits {
        hour_counts[time.hour() as usize] += 1;
    }
    
    println!("\nCommits by hour of day:");
    for (hour, &count) in hour_counts.iter().enumerate() {
        if count > 0 {
            let bar_length = (count * 30 / commits.len().max(1)).min(30);
            let bar = "█".repeat(bar_length);
            println!("  {:02}:00 {:3} {}", hour, count, bar.green());
        }
    }
    
    // Commit frequency
    if commits.len() > 1 {
        // Safe to unwrap: we just checked commits.len() > 1
        let first_time = commits.first().expect("commits should not be empty").1;
        let last_time = commits.last().expect("commits should not be empty").1;
        let total_duration = last_time.signed_duration_since(first_time);
        
        let days = total_duration.num_days().max(1);
        let avg_per_day = commits.len() as f64 / days as f64;
        
        println!("\nCommit frequency:");
        println!("  Average: {:.2} commits per day", avg_per_day);
        println!("  Total period: {} days", days);
    }
    
    print!("{}", formatter.double_separator());
}