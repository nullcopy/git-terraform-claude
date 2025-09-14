use git2::{Repository, Commit, Oid, Signature, Time};
use chrono::{DateTime, Utc, Duration, TimeZone};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::{Result, anyhow};

// Type alias for improved readability
type TimestampChange = (Oid, DateTime<Utc>, DateTime<Utc>);

pub struct GitOps {
    pub repo: Repository,
}

impl GitOps {
    pub fn new() -> Result<Self> {
        let repo = Repository::open_from_env()?;
        Ok(Self { repo })
    }
    
    pub fn get_current_branch(&self) -> Result<String> {
        let head = self.repo.head()?;
        Ok(head.shorthand().unwrap_or("HEAD").to_string())
    }
    
    pub fn get_commits_in_range(&self, range: Option<&str>) -> Result<Vec<Commit<'_>>> {
        let mut revwalk = self.repo.revwalk()?;
        
        match range {
            Some(range_spec) if range_spec.contains("..") => {
                let parts: Vec<&str> = range_spec.split("..").collect();
                if parts.len() != 2 {
                    return Err(anyhow!("Invalid range: {}", range_spec));
                }
                revwalk.push(self.repo.revparse_single(parts[1])?.id())?;
                revwalk.hide(self.repo.revparse_single(parts[0])?.id())?;
            }
            Some(range_spec) => {
                revwalk.push(self.repo.revparse_single(range_spec)?.id())?;
            }
            None => revwalk.push_head()?,
        }
        
        let commits: std::result::Result<Vec<_>, _> = revwalk
            .map(|oid| oid.and_then(|id| self.repo.find_commit(id)))
            .collect();
        
        let mut commits = commits?;
        commits.reverse(); // Return in chronological order
        Ok(commits)
    }
    
    pub fn filter_commits<'a>(
        &self,
        commits: Vec<Commit<'a>>,
        since: Option<&str>,
        until: Option<&str>,
        author: Option<&str>,
    ) -> Result<Vec<Commit<'a>>> {
        let since_time = since.map(parse_date).transpose()?;
        let until_time = until.map(parse_date).transpose()?;
        let author_re = author
            .map(|p| Regex::new(&p.replace("*", ".*")))
            .transpose()
            .map_err(|e| anyhow!("Parse error: Invalid author pattern: {}", e))?;
        
        Ok(commits.into_iter().filter(|c| {
            since_time.map_or(true, |t| commit_datetime(c) >= t) &&
            until_time.map_or(true, |t| commit_datetime(c) <= t) &&
            author_re.as_ref().map_or(true, |re| {
                c.author().name().is_some_and(|n| re.is_match(n)) ||
                c.author().email().is_some_and(|e| re.is_match(e))
            })
        }).collect())
    }
    
    pub fn shift_commits(
        &self,
        commits: &[Commit],
        shift: Duration,
        _dry_run: bool,
    ) -> Result<Vec<TimestampChange>> {
        Ok(commits.iter()
            .map(|commit| {
                let old_time = commit_datetime(commit);
                (commit.id(), old_time, old_time + shift)
            })
            .collect())
    }
    
    pub fn check_monotonic(&self, changes: &[TimestampChange]) -> bool {
        let mut times: Vec<_> = changes.iter().map(|(_, _, new)| *new).collect();
        times.sort();
        times.windows(2).all(|w| w[0] < w[1])
    }
    
    pub fn apply_timestamp_changes<F>(
        &self,
        changes: &[TimestampChange],
        mut progress_callback: F,
    ) -> Result<()> 
    where
        F: FnMut(&Oid),
    {
        let head = self.repo.head()?;
        
        let oid_to_new_time: HashMap<Oid, DateTime<Utc>> = changes
            .iter()
            .map(|(oid, _, new_time)| (*oid, *new_time))
            .collect();
        
        let oids_to_change: HashSet<Oid> = changes.iter().map(|(oid, _, _)| *oid).collect();
        
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        
        let all_commits: Vec<Oid> = revwalk
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Git error: {}", e))?;
        
        if !all_commits.iter().any(|oid| oids_to_change.contains(oid)) {
            return Ok(());
        }
        
        let mut new_oid_map: HashMap<Oid, Oid> = HashMap::new();
        let commits_to_rewrite: Vec<Oid> = all_commits.into_iter().rev().collect();
        
        // Rewrite commits from oldest to newest
        for commit_oid in commits_to_rewrite {
            let commit = self.repo.find_commit(commit_oid)?;
            
            let new_timestamp = oid_to_new_time.get(&commit_oid)
                .map(|new_time| {
                    progress_callback(&commit_oid);
                    Time::new(new_time.timestamp(), 0)
                })
                .unwrap_or_else(|| commit.time());
            
            let parent_ids: Vec<Oid> = commit.parent_ids()
                .map(|pid| new_oid_map.get(&pid).copied().unwrap_or(pid))
                .collect();
            
            let parents: Vec<Commit> = parent_ids
                .iter()
                .map(|oid| self.repo.find_commit(*oid))
                .collect::<std::result::Result<_, _>>()?;
            let parent_refs: Vec<&Commit> = parents.iter().collect();
            
            let new_oid = self.repo.commit(
                None,
                &create_signature_with_time(&commit.author(), &new_timestamp)?,
                &create_signature_with_time(&commit.committer(), &new_timestamp)?,
                commit.message().unwrap_or(""),
                &commit.tree()?,
                &parent_refs,
            )?;
            
            new_oid_map.insert(commit_oid, new_oid);
        }
        
        let head_target = head.target()
            .ok_or_else(|| git2::Error::from_str("HEAD has no target"))?;
        
        if let Some(&new_head_oid) = new_oid_map.get(&head_target) {
            self.repo.reference(
                head.name().ok_or_else(|| git2::Error::from_str("HEAD has no name"))?,
                new_head_oid,
                true,
                "git-terraform: shifted timestamps",
            )?;
        }
        
        Ok(())
    }
    
    /// Create a test commit with specified timestamp
    pub fn create_test_commit(&self, message: &str, timestamp: DateTime<Utc>) -> Result<Oid> {
        const TEST_FILE: &str = ".git-terraform-test-data.txt";
        
        // Get HEAD commit
        let head = self.repo.head()?;
        let parent_commit = head.peel_to_commit()?;
        let parent_tree = parent_commit.tree()?;
        
        // Read existing test file content or start empty
        let current_content = if let Some(entry) = parent_tree.get_path(Path::new(TEST_FILE)).ok() {
            let blob = self.repo.find_blob(entry.id())?;
            String::from_utf8_lossy(blob.content()).to_string()
        } else {
            String::new()
        };
        
        // Append the commit message as new content
        let new_content = format!("{}{}\n", current_content, message);
        
        // Create blob with new content
        let blob_oid = self.repo.blob(new_content.as_bytes())?;
        
        // Build new tree with modified file
        let mut tree_builder = self.repo.treebuilder(Some(&parent_tree))?;
        tree_builder.insert(TEST_FILE, blob_oid, 0o100644)?;
        let tree_oid = tree_builder.write()?;
        let new_tree = self.repo.find_tree(tree_oid)?;
        
        // Create signature with specified timestamp
        let time = Time::new(timestamp.timestamp(), 0);
        let sig = self.repo.signature()?;
        let author = create_signature_with_time(&sig, &time)?;
        let committer = author.clone();
        
        // Create the commit
        let commit_oid = self.repo.commit(
            Some("HEAD"),
            &author,
            &committer,
            message,
            &new_tree,
            &[&parent_commit],
        )?;
        
        Ok(commit_oid)
    }
    
    /// Create a commit that deletes the test file
    pub fn delete_test_file_commit(&self, message: &str, timestamp: DateTime<Utc>) -> Result<Oid> {
        const TEST_FILE: &str = ".git-terraform-test-data.txt";
        
        // Get HEAD commit
        let head = self.repo.head()?;
        let parent_commit = head.peel_to_commit()?;
        let parent_tree = parent_commit.tree()?;
        
        // Build new tree without the test file
        let mut tree_builder = self.repo.treebuilder(Some(&parent_tree))?;
        tree_builder.remove(TEST_FILE)?;
        let tree_oid = tree_builder.write()?;
        let new_tree = self.repo.find_tree(tree_oid)?;
        
        // Create signature with specified timestamp
        let time = Time::new(timestamp.timestamp(), 0);
        let sig = self.repo.signature()?;
        let author = create_signature_with_time(&sig, &time)?;
        let committer = author.clone();
        
        // Create the commit
        let commit_oid = self.repo.commit(
            Some("HEAD"),
            &author,
            &committer,
            message,
            &new_tree,
            &[&parent_commit],
        )?;
        
        Ok(commit_oid)
    }
}

/// Convert a git commit's timestamp to a DateTime<Utc>
fn commit_datetime(commit: &Commit) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(commit.time().seconds(), 0)
        .unwrap_or_else(Utc::now)
}

fn parse_date(date_str: &str) -> Result<DateTime<Utc>> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| Utc.from_utc_datetime(&dt))
        .or_else(|| parse_relative_date(date_str).map(|d| Utc::now() - d))
        .ok_or_else(|| anyhow!("Parse error: Invalid date format: {}", date_str))
}

fn parse_relative_date(date_str: &str) -> Option<Duration> {
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    
    if parts.len() >= 3 && parts[2] == "ago" {
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

fn create_signature_with_time(original: &Signature, time: &Time) -> Result<Signature<'static>> {
    Ok(Signature::new(
        original.name().unwrap_or(""),
        original.email().unwrap_or(""),
        time,
    )?)
}