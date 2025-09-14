use chrono::{DateTime, Utc, Duration};
use git2::Oid;

/// Handles density visualization for commit timelines
pub struct DensityRenderer {
    width: usize,
}

impl DensityRenderer {
    pub fn new(width: usize) -> Self {
        Self { width }
    }
    
    /// Generates a density visualization line for commits
    pub fn generate_line(
        &self,
        commits: &[(Oid, DateTime<Utc>, String)],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<char> {
        let mut density_line = vec![' '; self.width];
        
        let total_duration = end.signed_duration_since(start);
        if total_duration.num_seconds() == 0 {
            return density_line;
        }
        
        // Calculate density for each segment
        let segment_counts = self.calculate_segment_densities(commits, start, end);
        
        // Map densities to visualization characters
        self.apply_density_characters(&mut density_line, &segment_counts);
        
        density_line
    }
    
    /// Generates shifted density lines for before/after comparison
    pub fn generate_shifted_lines(
        &self,
        changes: &[(Oid, DateTime<Utc>, DateTime<Utc>)],
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
    ) -> (Vec<char>, Vec<char>) {
        // Generate old timeline density
        let old_commits: Vec<(Oid, DateTime<Utc>, String)> = changes.iter()
            .map(|(oid, old_time, _)| (*oid, *old_time, String::new()))
            .collect();
        let old_density = self.generate_line(&old_commits, min_time, max_time);
        
        // Generate new timeline density
        let new_commits: Vec<(Oid, DateTime<Utc>, String)> = changes.iter()
            .map(|(oid, _, new_time)| (*oid, *new_time, String::new()))
            .collect();
        let new_density = self.generate_line(&new_commits, min_time, max_time);
        
        (old_density, new_density)
    }
    
    /// Generates an arrow line showing the shift direction
    pub fn generate_arrow_line(
        &self,
        changes: &[(Oid, DateTime<Utc>, DateTime<Utc>)],
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
    ) -> String {
        if changes.is_empty() {
            return " ".repeat(self.width);
        }
        
        let total_duration = max_time.signed_duration_since(min_time);
        if total_duration.num_seconds() == 0 {
            return " ".repeat(self.width);
        }
        
        // Calculate average shift direction
        let mut total_shift = Duration::seconds(0);
        for (_, old_time, new_time) in changes {
            let shift = new_time.signed_duration_since(*old_time);
            total_shift += shift;
        }
        let avg_shift = total_shift / changes.len() as i32;
        let shifting_backward = avg_shift.num_seconds() < 0;
        
        // Get time bounds for arrow positions
        let old_times: Vec<DateTime<Utc>> = changes.iter().map(|(_, old, _)| *old).collect();
        let new_times: Vec<DateTime<Utc>> = changes.iter().map(|(_, _, new)| *new).collect();
        
        let old_min = *old_times.iter().min().expect("old_times non-empty");
        let old_max = *old_times.iter().max().expect("old_times non-empty");
        let new_min = *new_times.iter().min().expect("new_times non-empty");
        let new_max = *new_times.iter().max().expect("new_times non-empty");
        
        // Calculate positions matching how density bars are positioned
        let old_duration = old_max.signed_duration_since(old_min);
        let new_duration = new_max.signed_duration_since(new_min);
        
        // Start positions
        let old_offset = old_min.signed_duration_since(min_time);
        let old_start_ratio = old_offset.num_seconds() as f64 / total_duration.num_seconds() as f64;
        let old_start_pos = (old_start_ratio * self.width as f64) as usize;
        
        let new_offset = new_min.signed_duration_since(min_time);
        let new_start_ratio = new_offset.num_seconds() as f64 / total_duration.num_seconds() as f64;
        let new_start_pos = (new_start_ratio * self.width as f64) as usize;
        
        // End positions (start + width - 1)
        let old_duration_ratio = old_duration.num_seconds() as f64 / total_duration.num_seconds() as f64;
        let old_pattern_width = ((old_duration_ratio * self.width as f64) as usize).max(1);
        let old_end_pos = (old_start_pos + old_pattern_width - 1).min(self.width - 1);
        
        let new_duration_ratio = new_duration.num_seconds() as f64 / total_duration.num_seconds() as f64;
        let new_pattern_width = ((new_duration_ratio * self.width as f64) as usize).max(1);
        let new_end_pos = (new_start_pos + new_pattern_width - 1).min(self.width - 1);
        
        // Draw the arrow path
        let mut path_line = vec![' '; self.width];
        
        if shifting_backward {
            // Draw backward arrow
            let arrow_start = old_start_pos;
            let arrow_end = new_start_pos;
            
            if arrow_end < arrow_start {
                // Draw horizontal line
                for i in (arrow_end + 1)..arrow_start {
                    if i < self.width {
                        path_line[i] = '─';
                    }
                }
                
                // Add corner characters
                if arrow_end < self.width {
                    path_line[arrow_end] = '┌';
                }
                if arrow_start < self.width {
                    path_line[arrow_start] = '┘';
                }
            } else if arrow_end == arrow_start {
                // Same position, show vertical line
                if arrow_start < self.width {
                    path_line[arrow_start] = '│';
                }
            }
        } else {
            // Draw forward arrow
            let arrow_start = old_end_pos + 1;
            let arrow_end = new_end_pos + 1;
            
            if arrow_start < arrow_end && arrow_end < self.width {
                // Draw horizontal line
                for i in (arrow_start + 1)..arrow_end {
                    if i < self.width {
                        path_line[i] = '─';
                    }
                }
                
                // Add corner characters
                if arrow_start < self.width {
                    path_line[arrow_start] = '└';
                }
                if arrow_end < self.width {
                    path_line[arrow_end] = '┐';
                }
            } else if arrow_start == arrow_end {
                // Same position, show vertical line
                if arrow_start < self.width {
                    path_line[arrow_start] = '│';
                }
            }
        }
        
        path_line.into_iter().collect()
    }
    
    fn calculate_segment_densities(
        &self,
        commits: &[(Oid, DateTime<Utc>, String)],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<usize> {
        let segment_count = self.width.min(100); // Limit segments for performance
        let mut segment_counts = vec![0; segment_count];
        
        let total_duration = end.signed_duration_since(start);
        if total_duration.num_seconds() == 0 {
            return segment_counts;
        }
        
        for (_, time, _) in commits {
            let offset = time.signed_duration_since(start);
            let ratio = offset.num_seconds() as f64 / total_duration.num_seconds() as f64;
            let segment = ((ratio * segment_count as f64) as usize).min(segment_count - 1);
            segment_counts[segment] += 1;
        }
        
        segment_counts
    }
    
    fn apply_density_characters(&self, density_line: &mut [char], segment_counts: &[usize]) {
        let density_chars = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let max_count = segment_counts.iter().max().copied().unwrap_or(1).max(1);
        let segment_count = segment_counts.len();
        
        for i in 0..self.width {
            let segment = (i * segment_count) / self.width;
            let count = segment_counts[segment];
            let density_index = if count == 0 {
                0
            } else {
                ((count * 8) / max_count).clamp(1, 8)
            };
            density_line[i] = density_chars[density_index];
        }
    }
}