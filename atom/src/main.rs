use std::cmp::{max, Ordering};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fs::{read_dir, read_to_string};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, ensure, Context, Result};
use atom_syndication::{Content, Entry, Feed, FeedBuilder, Link, Person};
use chrono::{DateTime, Local};
use clap::Parser;
use git2::{Repository, Status, StatusOptions, StatusShow};
use lazy_static::lazy_static;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    /// Path to directory containing html
    #[clap(long)]
    html_dir: PathBuf,
    /// Path to git repository root
    #[clap(long)]
    repo_root: PathBuf,
}

lazy_static! {
    static ref EXCLUDE: HashSet<OsString> = {
        let mut m = HashSet::new();
        m.insert(OsStr::new("index.html").to_owned());
        m.insert(OsStr::new("about.html").to_owned());
        m.insert(OsStr::new("talks.html").to_owned());
        m
    };
}

/// Finds all the html posts inside the given directory and returns set of absolute paths
fn find_posts(dir: &Path) -> Result<HashSet<PathBuf>> {
    let mut ret = HashSet::new();
    for entry in read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(ext) = path.extension() {
            if ext != "html" {
                continue;
            }
        }

        if let Some(file) = path.file_name() {
            if EXCLUDE.contains(file) {
                continue;
            }
        }

        ret.insert(path.canonicalize().context("Failed to canonicalize path")?);
    }

    Ok(ret)
}

/// Reads out mtime on each post from git
///
/// Mostly stolen from https://github.com/rust-lang/git2-rs/issues/588#issuecomment-856757971
fn find_mtimes(posts: &HashSet<PathBuf>, repo_root: &Path) -> Result<HashMap<PathBuf, i64>> {
    let mut mtimes: HashMap<PathBuf, i64> = HashMap::new();
    let repo = Repository::open(repo_root)?;

    // Collect mtimes for checked in posts
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TIME)?;
    revwalk.push_head()?;
    for commit_id in revwalk {
        let commit_id = commit_id?;
        let commit = repo.find_commit(commit_id)?;
        // Ignore merge commits (2+ parents) because that's what 'git whatchanged' does.
        // Ignore commit with 0 parents (initial commit) because there's nothing to diff against
        if commit.parent_count() == 1 {
            let prev_commit = commit.parent(0)?;
            let tree = commit.tree()?;
            let prev_tree = prev_commit.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&prev_tree), Some(&tree), None)?;
            for delta in diff.deltas() {
                // Get path as stored in git
                let relative_path = delta
                    .new_file()
                    .path()
                    .ok_or_else(|| anyhow!("Delta missing path"))?;

                // Ignore non-html posts
                if let Some(ext) = relative_path.extension() {
                    if ext != "html" {
                        continue;
                    }
                }

                // Canonicalize the relative path to check against the posts we found
                let mut absolute_path = repo_root.to_owned();
                absolute_path.push(relative_path);
                absolute_path = match absolute_path.canonicalize() {
                    Ok(p) => p,
                    // Since we are walking all of the repo history, some files may
                    // have been deleted in the past. So ignore failures to canonicalize.
                    Err(_) => continue,
                };

                if !posts.contains(&absolute_path) {
                    continue;
                }

                let file_mod_time = commit.time();
                let unix_time = file_mod_time.seconds();
                mtimes
                    .entry(absolute_path)
                    .and_modify(|t| *t = max(*t, unix_time))
                    .or_insert(unix_time);
            }
        }
    }

    // Now collect mtimes for new (not checked-in yet) posts
    let mut status_opts = StatusOptions::new();
    status_opts
        .show(StatusShow::IndexAndWorkdir)
        .include_untracked(true);
    let statuses = repo.statuses(Some(&mut status_opts))?;
    for status in statuses
        .iter()
        .filter(|s| s.status().contains(Status::WT_NEW))
    {
        let relative_path = status.path().unwrap();
        if !relative_path.ends_with(".html") {
            continue;
        }

        let mut absolute_path = repo_root.to_owned();
        absolute_path.push(relative_path);
        absolute_path = absolute_path
            .canonicalize()
            .context("Failed to canonicalize untracked post")?;
        if !posts.contains(&absolute_path) {
            continue;
        }

        // Just use current time for now while the new post is uncommitted.
        // It'll get the commit timestamp once it's in the tree.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .try_into()?;
        mtimes.insert(absolute_path, now);
    }

    ensure!(posts.len() == mtimes.len(), "Did not locate all mtimes");

    Ok(mtimes)
}

fn unix_time_to_datetime(secs: i64) -> Result<DateTime<Local>> {
    let system = UNIX_EPOCH + Duration::from_secs(u64::try_from(secs)?);
    Ok(system.into())
}

/// Takes in a sorted slice of posts and maps posts to entries
fn build_entries(posts: &[(&PathBuf, &i64)]) -> Result<Vec<Entry>> {
    let mut ret = Vec::new();
    for (path, timestamp) in posts {
        let mut content = Content::default();
        let mut entry = Entry::default();

        content.set_base("https://dxuuu.xyz/".to_string());
        content.set_value(read_to_string(path).context("Failed to read html")?);
        content.set_content_type("html".to_string());

        entry.set_title(
            &*path
                .file_stem()
                .ok_or_else(|| anyhow!("{} has no filename", path.display()))?
                .to_string_lossy(),
        );
        entry.set_id(format!(
            "https://dxuuu.xyz/{}",
            &*path
                .file_name()
                .ok_or_else(|| anyhow!("{} has no filename", path.display()))?
                .to_string_lossy()
        ));
        entry.set_updated(unix_time_to_datetime(**timestamp)?);
        entry.set_content(content);

        ret.push(entry);
    }

    Ok(ret)
}

fn build_feed(mtimes: &HashMap<PathBuf, i64>) -> Result<Feed> {
    let mut posts = mtimes.iter().collect::<Vec<(&PathBuf, &i64)>>();
    posts.sort_by(|a, b| match a.1.partial_cmp(b.1).unwrap() {
        Ordering::Equal => a.0.partial_cmp(b.0).unwrap(),
        o => o,
    });
    let latest = posts.last().ok_or_else(|| anyhow!("No posts found"))?;
    let mut me = Person::default();
    me.set_name("Daniel Xu");
    let mut link = Link::default();
    link.set_href("https://dxuuu.xyz/atom.xml");
    link.set_rel("self");
    let entries = build_entries(&posts).context("Failed to build entries")?;

    Ok(FeedBuilder::default()
        .title("dxu's blog")
        .id("https://dxuuu.xyz/")
        .link(link)
        .author(me)
        .updated(unix_time_to_datetime(*latest.1)?)
        .entries(entries)
        .build())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let posts = find_posts(&args.html_dir).context("Failed to find posts")?;
    let mtimes = find_mtimes(&posts, &args.repo_root).context("Failed to find mtimes")?;
    let feed = build_feed(&mtimes).context("Failed to build atom feed")?;

    println!("{}", feed.to_string());

    Ok(())
}
