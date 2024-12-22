use std::cmp::{max, Ordering};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fs::{read_dir, read_to_string};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use atom_syndication::{Content, Entry, Feed, FeedBuilder, LinkBuilder, Person};
use chrono::{DateTime, Local};
use clap::Parser;
use git2::{Repository, StatusOptions, StatusShow};
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

/// Converts a canonicalized path to a markdown post to a canonicalized path to an html file
fn md_to_html(post: &Path) -> Result<PathBuf> {
    if post.extension() != Some(OsStr::new("md")) {
        bail!("Path={} is not a markdown file", post.display());
    }

    let mut parts = post.components().collect::<Vec<_>>();
    if parts.len() < 2 {
        bail!("Path={} too short: {} < 2", post.display(), parts.len());
    }

    // Change immediate parent directory to html/
    let len = parts.len();
    parts[len - 2] = Component::Normal(OsStr::new("html"));
    let mut mapped = parts.iter().collect::<PathBuf>();
    mapped.set_extension("html");

    Ok(mapped)
}

/// Return all the entries in `posts` but not in `mtimes`
fn diff_posts_mtimes(posts: &HashSet<PathBuf>, mtimes: &HashMap<PathBuf, i64>) -> Vec<PathBuf> {
    posts
        .iter()
        .filter(|p| mtimes.get(*p).is_none())
        .cloned()
        .collect()
}

/// Reads out mtime on each post from git
///
/// Mostly stolen from https://github.com/rust-lang/git2-rs/issues/588#issuecomment-856757971 .
///
/// Basically we look at all the html files, map them to a markdown file, and then
/// look for the last edit time on the markdown file using git history.
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

                // Ignore non-markdown files
                match relative_path.extension() {
                    Some(ext) if ext == "md" => {}
                    Some(_) | None => continue,
                }

                // Ignore non-posts
                if relative_path.parent() != Some(Path::new("src")) {
                    continue;
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

                let html_path =
                    md_to_html(&absolute_path).context("Failed to map md file to html")?;
                if !posts.contains(&html_path) {
                    continue;
                }

                let file_mod_time = commit.time();
                let unix_time = file_mod_time.seconds();
                mtimes
                    .entry(html_path)
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
    for status in statuses.iter() {
        let relative_path = status.path().unwrap();
        if !relative_path.ends_with(".md") {
            continue;
        }

        let mut absolute_path = repo_root.to_owned();
        absolute_path.push(relative_path);
        absolute_path = absolute_path
            .canonicalize()
            .context("Failed to canonicalize untracked post")?;
        let html_path = md_to_html(&absolute_path).context("Failed to map md file to html")?;
        if !posts.contains(&html_path) {
            continue;
        }

        // Just use current time for now while the new post is uncommitted.
        // It'll get the commit timestamp once it's in the tree.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .try_into()?;
        mtimes.insert(html_path, now);
    }

    if posts.len() != mtimes.len() {
        let diff = diff_posts_mtimes(posts, &mtimes);
        return Err(
            anyhow!("Did not locate all mtimes").context(format!("Did not find: {:?}", diff))
        );
    }

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
        let uri = format!(
            "https://dxuuu.xyz/{}",
            &*path
                .file_name()
                .ok_or_else(|| anyhow!("{} has no filename", path.display()))?
                .to_string_lossy()
        );

        content.set_base("https://dxuuu.xyz/".to_string());
        content.set_value(read_to_string(path).context("Failed to read html")?);
        content.set_content_type("html".to_string());

        entry.set_title(
            &*path
                .file_stem()
                .ok_or_else(|| anyhow!("{} has no filename", path.display()))?
                .to_string_lossy(),
        );
        entry.set_id(&uri);
        entry.set_links(&[LinkBuilder::default().href(&uri).build()]);
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
    let feed_link = LinkBuilder::default()
        .href("https://dxuuu.xyz/atom.xml")
        .rel("self")
        .build();
    let link = LinkBuilder::default().href("https://dxuuu.xyz").build();
    let entries = build_entries(&posts).context("Failed to build entries")?;

    Ok(FeedBuilder::default()
        .title("dxu's blog")
        .id("https://dxuuu.xyz/")
        .link(feed_link)
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
