use crate::duration_format;
use chrono::{DateTime, Utc};
use colored::*;
use git2::Oid;

mod density;
mod markers;

use density::DensityRenderer;
use markers::{generate_dual_scale_markers, format_date_labels};

/// Main timeline struct that coordinates visualization of git commit timelines
pub struct Timeline {
    width: usize,
}

impl Timeline {
    pub fn new(width: usize) -> Self {
        Self { width }
    }
    
    /// Renders a preview of changes that will be applied to commits
    pub fn render_changes(
        &self,
        repo: &git2::Repository,
        changes: &[(Oid, DateTime<Utc>, DateTime<Utc>)],
    ) -> String {
        if changes.is_empty() {
            return "No commits to display".to_string();
        }
        
        let mut output = String::new();
        
        // Header
        self.append_header(&mut output, "TIMELINE PREVIEW");
        
        // List commits to be modified
        self.append_commit_list(&mut output, repo, changes);
        
        output.push_str(&format!("\n{}\n", "─".repeat(self.width)));
        
        // Render comparative timelines
        self.append_comparative_timeline(&mut output, changes);
        
        output.push_str(&format!("{}\n", "═".repeat(self.width)));
        
        output
    }
    
    /// Renders an analysis view of commit timeline
    pub fn render_analysis(
        &self,
        commits: &[(Oid, DateTime<Utc>, String)], // (oid, time, author)
    ) -> String {
        if commits.is_empty() {
            return "No commits to analyze".to_string();
        }
        
        let mut output = String::new();
        
        // Header
        self.append_header(&mut output, "COMMIT TIMELINE ANALYSIS");
        
        // Statistics
        self.append_statistics(&mut output, commits);
        
        // Visual timeline
        self.append_compact_timeline(&mut output, commits);
        
        output.push_str(&format!("{}\n", "═".repeat(self.width)));
        
        output
    }
    
    fn append_commit_list(
        &self,
        output: &mut String,
        repo: &git2::Repository,
        changes: &[(Oid, DateTime<Utc>, DateTime<Utc>)],
    ) {
        output.push_str("\nCommits to be modified:\n");
        
        for (oid, old_time, new_time) in changes.iter().take(10) {
            let shift = new_time.signed_duration_since(*old_time);
            let shift_str = if shift.num_seconds() > 0 {
                duration_format::human_readable_with_sign(&shift).green()
            } else {
                duration_format::human_readable_with_sign(&shift).red()
            };
            
            let message = repo.find_commit(*oid)
                .ok()
                .and_then(|c| c.summary().map(|s| s.to_string()))
                .unwrap_or_else(|| "<no message>".to_string());
            
            output.push_str(&format!("  {} {} {}\n",
                format!("{:.8}", oid).cyan(),
                shift_str,
                message.dimmed()
            ));
        }
        
        if changes.len() > 10 {
            output.push_str(&format!("  ... and {} more commits\n", changes.len() - 10));
        }
    }
    
    fn append_statistics(&self, output: &mut String, commits: &[(Oid, DateTime<Utc>, String)]) {
        let total_commits = commits.len();
        let first_commit = commits.first().expect("commits verified non-empty").1;
        let last_commit = commits.last().expect("commits verified non-empty").1;
        let total_span = last_commit.signed_duration_since(first_commit);
        
        output.push_str(&format!("Total commits: {}\n", total_commits));
        output.push_str(&format!("Time span: {} to {}\n", 
            first_commit.format("%Y-%m-%d %H:%M"), 
            last_commit.format("%Y-%m-%d %H:%M")
        ));
        output.push_str(&format!("Duration: {}\n", duration_format::human_readable(&total_span)));
    }
    
    fn append_compact_timeline(&self, output: &mut String, commits: &[(Oid, DateTime<Utc>, String)]) {
        let first_time = commits.first().expect("commits verified non-empty").1;
        let last_time = commits.last().expect("commits verified non-empty").1;
        
        // Create density visualization
        let density_renderer = DensityRenderer::new(self.width);
        let density_line = density_renderer.generate_line(commits, first_time, last_time);
        
        // Generate time markers
        let (minor_markers, major_markers) = generate_dual_scale_markers(first_time, last_time, self.width);
        
        // Render the timeline
        let density_str: String = density_line.iter().collect();
        output.push_str(&format!("{}\n", density_str));
        
        // Add date labels
        if !minor_markers.is_empty() {
            output.push_str(&format_date_labels(&minor_markers, self.width));
        }
        if !major_markers.is_empty() {
            output.push_str(&format_date_labels(&major_markers, self.width));
        }
    }
    
    fn append_comparative_timeline(
        &self,
        output: &mut String,
        changes: &[(Oid, DateTime<Utc>, DateTime<Utc>)],
    ) {
        // Find time range
        let mut all_times: Vec<DateTime<Utc>> = Vec::new();
        for (_, old, new) in changes {
            all_times.push(*old);
            all_times.push(*new);
        }
        all_times.sort();
        
        let min_time = all_times.first().expect("all_times verified non-empty");
        let max_time = all_times.last().expect("all_times verified non-empty");
        
        let timeline_width = self.width - 6; // Account for "Old: " or "New: " prefix
        
        // Generate density visualizations
        let density_renderer = DensityRenderer::new(timeline_width);
        let (old_density, new_density) = density_renderer.generate_shifted_lines(
            changes, *min_time, *max_time
        );
        
        // Generate time markers
        let (minor_markers, major_markers) = generate_dual_scale_markers(*min_time, *max_time, timeline_width);
        
        // Render comparative timelines
        output.push_str("\nComparative Timeline:\n\n");
        
        let old_str: String = old_density.iter().collect();
        let new_str: String = new_density.iter().collect();
        
        output.push_str(&format!("Old:  {}\n", old_str.red()));
        
        // Add shift arrow
        let arrow_line = density_renderer.generate_arrow_line(changes, *min_time, *max_time);
        output.push_str(&format!("      {}\n", arrow_line));
        
        output.push_str(&format!("New:  {}\n", new_str.green()));
        
        // Add date labels with proper indentation
        if !minor_markers.is_empty() {
            let label_str = format_date_labels(&minor_markers, timeline_width);
            self.append_indented(output, &label_str, "      ");
        }
        if !major_markers.is_empty() {
            let label_str = format_date_labels(&major_markers, timeline_width);
            self.append_indented(output, &label_str, "      ");
        }
    }
    
    fn append_header(&self, output: &mut String, title: &str) {
        output.push_str(&format!("\n{}\n", "═".repeat(self.width)));
        output.push_str(&format!("{:^width$}\n", title, width = self.width));
        output.push_str(&format!("{}\n", "─".repeat(self.width)));
    }
    
    fn append_indented(&self, output: &mut String, text: &str, indent: &str) {
        for line in text.lines() {
            output.push_str(indent);
            output.push_str(line);
            output.push('\n');
        }
    }
}