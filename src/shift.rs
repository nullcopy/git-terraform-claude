use anyhow::Result;
use crate::common::{GitContext, CommitFilterConfig, ProgressIndicator};
use crate::duration_format;
use crate::error::TerraformError;
use crate::time_parser::parse_time_shift;
use crate::timeline::Timeline;
use chrono::{DateTime, Utc};
use colored::*;
use dialoguer::Confirm;

/// Configuration for the shift operation
pub struct ShiftConfig<'a> {
    /// Time shift amount as a string (e.g., "+3d", "-2h")
    pub shift_amount: &'a str,
    /// Optional commit range specification
    pub range: Option<&'a str>,
    /// Filter commits after this date
    pub since: Option<&'a str>,
    /// Filter commits before this date
    pub until: Option<&'a str>,
}

impl<'a> ShiftConfig<'a> {
    /// Create a new ShiftConfig with the given values
    pub fn new(
        shift_amount: &'a str,
        range: Option<&'a str>,
        since: Option<&'a str>,
        until: Option<&'a str>,
    ) -> Self {
        Self {
            shift_amount,
            range,
            since,
            until,
        }
    }
}

/// Options controlling how the shift operation behaves
pub struct ShiftOptions {
    /// Skip the preview and confirmation prompt
    pub no_preview: bool,
    /// Allow creating non-monotonic timelines
    pub allow_non_monotonic: bool,
    /// Width for timeline visualization
    pub timeline_width: usize,
    /// Enable verbose output
    pub verbose: bool,
}

impl ShiftOptions {
    /// Create a new ShiftOptions with the given values
    pub fn new(
        no_preview: bool,
        allow_non_monotonic: bool,
        timeline_width: usize,
        verbose: bool,
    ) -> Self {
        Self {
            no_preview,
            allow_non_monotonic,
            timeline_width,
            verbose,
        }
    }
}

pub fn execute_shift(
    config: ShiftConfig,
    options: ShiftOptions,
) -> Result<()> {
    // Parse time shift
    let shift = parse_time_shift(config.shift_amount)?;
    
    if options.verbose {
        println!("Parsed time shift: {}", duration_format::compact(&shift));
    }
    
    // Initialize Git context
    let git_context = GitContext::new()?;
    git_context.print_branch("Operating on branch");
    
    // Get filtered commits using common functionality
    let filter_config = CommitFilterConfig::new(config.range, config.since, config.until);
    let filtered = git_context.get_filtered_commits(&filter_config, options.verbose)?;
    
    println!("Found {} commits matching criteria", filtered.len());
    
    // Use all filtered commits
    let commits_to_shift = filtered;
    
    if commits_to_shift.is_empty() {
        println!("No commits selected for shifting");
        return Ok(());
    }
    
    // Calculate changes
    let changes = git_context.git_ops.shift_commits(&commits_to_shift, shift, true)?;
    
    // Check for non-monotonic timeline
    check_timeline_monotonicity(&git_context.git_ops, &changes, options.allow_non_monotonic)?;
    
    // Preview and confirm changes if requested
    if !options.no_preview {
        preview_and_confirm(&git_context.git_ops, &changes, options.timeline_width)?;
    }
    
    // Apply changes
    apply_changes(&git_context.git_ops, &changes, options.verbose)?;
    
    Ok(())
}


/// Check if the timeline will be monotonic after changes
fn check_timeline_monotonicity(
    git_ops: &crate::git_ops::GitOps,
    changes: &[(git2::Oid, DateTime<Utc>, DateTime<Utc>)],
    allow_non_monotonic: bool,
) -> Result<()> {
    if !git_ops.check_monotonic(changes) {
        if !allow_non_monotonic {
            eprintln!("{}", "Warning: This operation would create a non-monotonic timeline!".red());
            eprintln!("Use --allow-non-monotonic to proceed anyway");
            return Err(TerraformError::NonMonotonicTimeline.into());
        }
        println!("{}", "Warning: Creating non-monotonic timeline as requested".yellow());
    }
    Ok(())
}

/// Preview changes and prompt for confirmation
fn preview_and_confirm(
    git_ops: &crate::git_ops::GitOps,
    changes: &[(git2::Oid, DateTime<Utc>, DateTime<Utc>)],
    timeline_width: usize,
) -> Result<()> {
    let timeline = Timeline::new(timeline_width);
    println!("{}", timeline.render_changes(&git_ops.repo, changes));
    
    let proceed = Confirm::new()
        .with_prompt("Do you want to apply these changes?")
        .default(false)
        .interact()
        .map_err(|_| anyhow::Error::from(TerraformError::UserCancelled))?;
    
    if !proceed {
        return Err(TerraformError::UserCancelled.into());
    }
    Ok(())
}

/// Apply the timestamp changes to the repository
fn apply_changes(
    git_ops: &crate::git_ops::GitOps,
    changes: &[(git2::Oid, DateTime<Utc>, DateTime<Utc>)],
    verbose: bool,
) -> Result<()> {
    let progress = ProgressIndicator::new(changes.len() as u64, "Applying timestamp changes...");
    
    // Apply the actual timestamp changes using git operations
    git_ops.apply_timestamp_changes(changes, |oid| {
        if verbose {
            progress.println(format!("Updating commit {:.8}...", oid));
        }
        progress.inc();
    })?;
    
    progress.finish("All changes applied successfully!");
    
    println!("\n{}", "✓ Commits have been shifted successfully".green());
    println!("{}", "Note: GPG signatures have been removed from modified commits".yellow());
    
    Ok(())
}