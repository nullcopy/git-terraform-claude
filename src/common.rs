use anyhow::Result;
use crate::error::TerraformError;
use crate::git_ops::GitOps;
use chrono::{DateTime, Utc};
use colored::*;
use git2::Commit;
use indicatif::{ProgressBar, ProgressStyle};

/// Common configuration for operations that work with git commits
pub struct CommitFilterConfig<'a> {
    pub range: Option<&'a str>,
    pub since: Option<&'a str>,
    pub until: Option<&'a str>,
}

impl<'a> CommitFilterConfig<'a> {
    pub fn new(range: Option<&'a str>, since: Option<&'a str>, until: Option<&'a str>) -> Self {
        Self { range, since, until }
    }
}

/// Common git operations setup and commit retrieval
pub struct GitContext {
    pub git_ops: GitOps,
    pub current_branch: String,
}

impl GitContext {
    /// Initialize git operations and get current branch
    pub fn new() -> Result<Self> {
        let git_ops = GitOps::new()?;
        let current_branch = git_ops.get_current_branch()?;
        Ok(Self { git_ops, current_branch })
    }
    
    /// Print the operating branch with standard formatting
    pub fn print_branch(&self, prefix: &str) {
        println!("{}: {}", prefix, self.current_branch.green());
    }
    
    /// Get filtered commits based on the provided configuration
    pub fn get_filtered_commits<'a>(
        &'a self,
        config: &CommitFilterConfig,
        verbose: bool,
    ) -> Result<Vec<Commit<'a>>> {
        // Log range if verbose
        if let Some(ref range) = config.range {
            if verbose {
                println!("Using range: {}", range);
            }
        }
        
        // Get commits
        let commits = self.git_ops.get_commits_in_range(config.range)?;
        
        if commits.is_empty() {
            return Err(TerraformError::NoMatchingCommits.into());
        }
        
        // Apply filters
        let filtered = self.git_ops.filter_commits(
            commits, 
            config.since, 
            config.until, 
            None
        )?;
        
        if filtered.is_empty() {
            return Err(TerraformError::NoMatchingCommits.into());
        }
        
        Ok(filtered)
    }
}

/// Convert a git2 commit timestamp to DateTime<Utc>
pub fn commit_to_datetime(commit: &Commit) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(commit.time().seconds(), 0)
        .unwrap_or_else(Utc::now)
}

/// Formatting utilities for consistent output
pub struct OutputFormatter {
    width: usize,
}

impl OutputFormatter {
    pub fn new(width: usize) -> Self {
        Self { width }
    }
    
    /// Create a header section with consistent formatting
    pub fn header(&self, title: &str) -> String {
        format!("\n{}\n{:^width$}\n{}\n",
            "═".repeat(self.width),
            title,
            "─".repeat(self.width),
            width = self.width
        )
    }
    
    /// Create a double separator line
    pub fn double_separator(&self) -> String {
        format!("{}\n", "═".repeat(self.width))
    }
}

/// Progress bar helper for consistent progress indication
pub struct ProgressIndicator {
    bar: ProgressBar,
}

impl ProgressIndicator {
    /// Create a new progress bar with standard formatting
    pub fn new(total: u64, message: &str) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .expect("valid progress bar template")
                .progress_chars("##-"),
        );
        bar.set_message(message.to_string());
        Self { bar }
    }
    
    /// Increment the progress bar
    pub fn inc(&self) {
        self.bar.inc(1);
    }
    
    /// Print a message while the progress bar is active (for verbose output)
    pub fn println(&self, msg: String) {
        self.bar.println(msg);
    }
    
    /// Complete the progress bar with a success message
    pub fn finish(&self, message: &str) {
        self.bar.finish_with_message(message.to_string());
    }
}