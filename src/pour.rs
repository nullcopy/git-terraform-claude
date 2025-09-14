use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, Timelike};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};

use crate::git_ops::GitOps;
use crate::timeline::Timeline;

pub fn execute_pour(
    count: usize,
    message_prefix: &str,
    timeline_width: usize,
    verbose: bool,
) -> Result<()> {
    if count == 0 {
        return Err(anyhow!("Count must be greater than 0"));
    }
    
    let git_ops = GitOps::new()?;
    
    // Get current time and start of today
    let now = Utc::now();
    let start_of_today = now
        .with_hour(0).unwrap()
        .with_minute(0).unwrap()
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();
    
    // Calculate time interval between commits
    let total_duration = now.signed_duration_since(start_of_today);
    let interval = total_duration / count as i32;
    
    println!("{}", "Pour Test Commits".green().bold());
    println!("Creating {} test commits distributed from {} to {}",
             count.to_string().cyan(),
             start_of_today.format("%H:%M:%S").to_string().yellow(),
             now.format("%H:%M:%S").to_string().yellow());
    
    // Generate timestamps for each commit
    let mut timestamps = Vec::new();
    for i in 0..count {
        let timestamp = start_of_today + (interval * i as i32);
        timestamps.push(timestamp);
    }
    
    // Show timeline preview
    if verbose {
        println!("\n{}", "Preview:".bold());
        let timeline = Timeline::new(timeline_width);
        let preview_commits: Vec<(git2::Oid, DateTime<Utc>, String)> = timestamps.iter()
            .enumerate()
            .map(|(i, ts)| {
                // Create dummy OIDs for preview
                let dummy_oid = git2::Oid::from_bytes(&[0; 20]).unwrap();
                let author = format!("Commit {}/{}", i + 1, count);
                (dummy_oid, *ts, author)
            })
            .collect();
        println!("{}", timeline.render_analysis(&preview_commits));
    }
    
    // Create progress bar
    let progress_bar = ProgressBar::new(count as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    
    // Create commits
    for i in 0..count {
        let message = if message_prefix.is_empty() {
            format!("Test commit {}/{}", i + 1, count)
        } else {
            format!("{}Test commit {}/{}", message_prefix, i + 1, count)
        };
        
        // For the last commit, delete the test file instead of appending to it
        if i == count - 1 {
            git_ops.delete_test_file_commit(&message, timestamps[i])?;
        } else {
            git_ops.create_test_commit(&message, timestamps[i])?;
        }
        
        progress_bar.inc(1);
        progress_bar.set_message(format!("Created: {}", message));
    }
    
    progress_bar.finish_with_message("Done!");
    
    println!("\n{} Created {} test commits", "✓".green().bold(), count);
    println!("Test data file: {} (deleted in final commit)", ".git-terraform-test-data.txt".cyan());
    
    // Show final timeline
    println!("\n{}", "Final Timeline:".bold());
    let final_commits = git_ops.get_commits_in_range(Some(&format!("HEAD~{}", count)))?;
    let timeline = Timeline::new(timeline_width);
    let timeline_data = final_commits.iter()
        .map(|c| {
            let time = DateTime::<Utc>::from_timestamp(c.time().seconds(), 0)
                .unwrap_or_else(Utc::now);
            let author = c.author().name().unwrap_or("Unknown").to_string();
            (c.id(), time, author)
        })
        .collect::<Vec<_>>();
    println!("{}", timeline.render_analysis(&timeline_data));
    
    Ok(())
}