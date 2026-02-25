use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use walkdir::WalkDir;

use crate::error::{Result, XurlError};
use crate::model::{ResolvedSkill, SkillResolutionMeta, SkillsSourceKind};
use crate::uri::SkillsUri;

#[derive(Debug, Clone)]
pub struct SkillsProvider {
    root: PathBuf,
    cache_root: PathBuf,
    github_base_url: Option<String>,
}

impl SkillsProvider {
    pub fn new(root: impl Into<PathBuf>, cache_root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cache_root: cache_root.into(),
            github_base_url: std::env::var("XURL_SKILLS_GITHUB_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
        }
    }

    #[cfg(test)]
    pub fn with_github_base_url(mut self, github_base_url: impl Into<String>) -> Self {
        self.github_base_url = Some(github_base_url.into());
        self
    }

    pub fn resolve(&self, uri: &SkillsUri) -> Result<ResolvedSkill> {
        match uri {
            SkillsUri::Local { skill_name } => self.resolve_local(uri, skill_name),
            SkillsUri::Github {
                owner,
                repo,
                skill_path,
            } => self.resolve_github(uri, owner, repo, skill_path.as_deref()),
        }
    }

    fn resolve_local(&self, uri: &SkillsUri, skill_name: &str) -> Result<ResolvedSkill> {
        let path = self.root.join(skill_name).join("SKILL.md");
        if !path.exists() {
            return Err(XurlError::SkillNotFound {
                uri: uri.as_string(),
            });
        }

        let content = read_skill_file(&path)?;

        Ok(ResolvedSkill {
            uri: uri.as_string(),
            source_kind: SkillsSourceKind::Local,
            skill_name: skill_name.to_string(),
            source: path.display().to_string(),
            resolved_path: format!("{skill_name}/SKILL.md"),
            content,
            metadata: SkillResolutionMeta::default(),
        })
    }

    fn resolve_github(
        &self,
        uri: &SkillsUri,
        owner: &str,
        repo: &str,
        skill_path: Option<&str>,
    ) -> Result<ResolvedSkill> {
        fs::create_dir_all(&self.cache_root).map_err(|source| XurlError::Io {
            path: self.cache_root.clone(),
            source,
        })?;

        let repo_dir = self.cache_root.join(cache_dir_name(owner, repo));
        let remote_url = self.github_remote_url(owner, repo);
        self.sync_repo(&repo_dir, &remote_url)?;

        if let Some(skill_path) = skill_path {
            let relative_skill_file = normalize_skill_file_path(skill_path)
                .ok_or_else(|| XurlError::InvalidSkillsUri(uri.as_string()))?;
            return self.resolve_github_from_relative(uri, repo, &repo_dir, &relative_skill_file);
        }

        for candidate in [
            PathBuf::from("SKILL.md"),
            PathBuf::from("skills").join(repo).join("SKILL.md"),
        ] {
            let absolute = repo_dir.join(&candidate);
            if absolute.exists() {
                return self.resolve_github_from_relative(uri, repo, &repo_dir, &candidate);
            }
        }

        let candidates = collect_skill_candidates(&repo_dir)?;
        match candidates.as_slice() {
            [] => Err(XurlError::SkillNotFound {
                uri: uri.as_string(),
            }),
            [candidate] => {
                self.resolve_github_from_relative(uri, repo, &repo_dir, Path::new(candidate))
            }
            _ => Err(XurlError::SkillSelectionRequired {
                uri: uri.as_string(),
                candidates: candidates
                    .iter()
                    .map(|candidate| candidate_to_uri(owner, repo, candidate))
                    .collect(),
            }),
        }
    }

    fn resolve_github_from_relative(
        &self,
        uri: &SkillsUri,
        repo: &str,
        repo_dir: &Path,
        relative_skill_file: &Path,
    ) -> Result<ResolvedSkill> {
        let absolute_skill_file = repo_dir.join(relative_skill_file);
        if !absolute_skill_file.exists() {
            return Err(XurlError::SkillNotFound {
                uri: uri.as_string(),
            });
        }

        let content = read_skill_file(&absolute_skill_file)?;
        let relative = relative_skill_file.to_string_lossy().replace('\\', "/");

        Ok(ResolvedSkill {
            uri: uri.as_string(),
            source_kind: SkillsSourceKind::Github,
            skill_name: skill_name_from_relative(repo, &relative),
            source: absolute_skill_file.display().to_string(),
            resolved_path: relative,
            content,
            metadata: SkillResolutionMeta::default(),
        })
    }

    fn github_remote_url(&self, owner: &str, repo: &str) -> String {
        if let Some(base) = &self.github_base_url {
            return format!("{}/{owner}/{repo}.git", base.trim_end_matches('/'),);
        }

        format!("https://github.com/{owner}/{repo}.git")
    }

    fn sync_repo(&self, repo_dir: &Path, remote_url: &str) -> Result<()> {
        if repo_dir.join(".git").exists() {
            run_git(
                [
                    OsStr::new("-C"),
                    repo_dir.as_os_str(),
                    OsStr::new("fetch"),
                    OsStr::new("--depth=1"),
                    OsStr::new("origin"),
                ],
                &self.cache_root,
            )?;
            run_git(
                [
                    OsStr::new("-C"),
                    repo_dir.as_os_str(),
                    OsStr::new("reset"),
                    OsStr::new("--hard"),
                    OsStr::new("FETCH_HEAD"),
                ],
                &self.cache_root,
            )?;
            return Ok(());
        }

        if repo_dir.exists() {
            return Err(XurlError::InvalidMode(format!(
                "skills cache path exists but is not a git repository: {}",
                repo_dir.display()
            )));
        }

        if let Some(parent) = repo_dir.parent() {
            fs::create_dir_all(parent).map_err(|source| XurlError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        run_git(
            [
                OsStr::new("clone"),
                OsStr::new("--filter=blob:none"),
                OsStr::new("--depth=1"),
                OsStr::new(remote_url),
                repo_dir.as_os_str(),
            ],
            &self.cache_root,
        )?;

        Ok(())
    }
}

fn read_skill_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| XurlError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    if bytes.is_empty() {
        return Err(XurlError::EmptySkillFile {
            path: path.to_path_buf(),
        });
    }

    String::from_utf8(bytes).map_err(|_| XurlError::NonUtf8SkillFile {
        path: path.to_path_buf(),
    })
}

fn run_git<const N: usize>(args: [&OsStr; N], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                XurlError::CommandNotFound {
                    command: "git".to_string(),
                }
            } else {
                XurlError::Io {
                    path: PathBuf::from("git"),
                    source,
                }
            }
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    let command = format!(
        "git {}",
        args.iter()
            .map(|item| item.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(XurlError::GitCommandFailed {
        command,
        code: output.status.code(),
        stderr,
    })
}

fn normalize_skill_file_path(path: &str) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\') {
            return None;
        }
        normalized.push(segment);
    }

    let ends_with_skill_file = normalized
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "SKILL.md");
    if !ends_with_skill_file {
        normalized.push("SKILL.md");
    }

    Some(normalized)
}

fn skill_name_from_relative(repo: &str, relative_skill_file: &str) -> String {
    let path = Path::new(relative_skill_file);
    let parent = path.parent().and_then(Path::to_str).unwrap_or_default();
    if parent.is_empty() || parent == "." {
        return repo.to_string();
    }

    Path::new(parent)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| repo.to_string())
}

fn collect_skill_candidates(repo_dir: &Path) -> Result<Vec<String>> {
    let mut candidates = Vec::new();

    for entry in WalkDir::new(repo_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "SKILL.md" {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(repo_dir)
            .map_err(|_| {
                XurlError::InvalidMode("failed to strip skill candidate prefix".to_string())
            })?
            .to_string_lossy()
            .replace('\\', "/");
        candidates.push(relative);
    }

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn cache_dir_name(owner: &str, repo: &str) -> String {
    fn sanitize(value: &str) -> String {
        value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
    }

    format!("github-com-{}-{}", sanitize(owner), sanitize(repo))
}

fn candidate_to_uri(owner: &str, repo: &str, relative_skill_file: &str) -> String {
    let path = Path::new(relative_skill_file);
    let parent = path.parent().and_then(Path::to_str).unwrap_or_default();
    if parent.is_empty() || parent == "." {
        format!("skills://github.com/{owner}/{repo}")
    } else {
        format!("skills://github.com/{owner}/{repo}/{parent}")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::SkillsProvider;
    use crate::error::XurlError;
    use crate::uri::SkillsUri;

    #[test]
    fn resolve_local_skill_success() {
        let dir = tempdir().expect("tempdir");
        let skill_dir = dir.path().join("skills/xurl");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: xurl\ndescription: test\n---\n\n# xurl\n",
        )
        .expect("write");

        let provider = SkillsProvider::new(dir.path().join("skills"), dir.path().join("cache"));
        let resolved = provider
            .resolve(&SkillsUri::parse("skills://xurl").expect("parse"))
            .expect("resolve");

        assert_eq!(resolved.skill_name, "xurl");
        assert!(resolved.content.contains("name: xurl"));
        assert_eq!(resolved.resolved_path, "xurl/SKILL.md");
    }

    #[test]
    fn resolve_local_skill_not_found() {
        let dir = tempdir().expect("tempdir");
        let provider = SkillsProvider::new(dir.path().join("skills"), dir.path().join("cache"));

        let err = provider
            .resolve(&SkillsUri::parse("skills://missing").expect("parse"))
            .expect_err("must fail");
        assert!(matches!(err, XurlError::SkillNotFound { .. }));
    }

    #[test]
    fn resolve_github_skill_by_path() {
        let dir = tempdir().expect("tempdir");
        let remotes = dir.path().join("remotes");
        create_git_remote(
            &remotes,
            "Xuanwo",
            "xurl",
            &[('s', "skills/xurl/SKILL.md", "# xurl\n")],
        );

        let provider = SkillsProvider::new(dir.path().join("local"), dir.path().join("cache"))
            .with_github_base_url(format!("file://{}", remotes.display()));

        let resolved = provider
            .resolve(
                &SkillsUri::parse("skills://github.com/Xuanwo/xurl/skills/xurl").expect("parse"),
            )
            .expect("resolve");

        assert_eq!(resolved.skill_name, "xurl");
        assert_eq!(resolved.resolved_path, "skills/xurl/SKILL.md");
        assert!(resolved.content.contains("# xurl"));
    }

    #[test]
    fn resolve_github_skill_reports_candidates() {
        let dir = tempdir().expect("tempdir");
        let remotes = dir.path().join("remotes");
        create_git_remote(
            &remotes,
            "Xuanwo",
            "xurl",
            &[
                ('a', "skills/first/SKILL.md", "# first\n"),
                ('b', "skills/second/SKILL.md", "# second\n"),
            ],
        );

        let provider = SkillsProvider::new(dir.path().join("local"), dir.path().join("cache"))
            .with_github_base_url(format!("file://{}", remotes.display()));

        let err = provider
            .resolve(&SkillsUri::parse("skills://github.com/Xuanwo/xurl").expect("parse"))
            .expect_err("must fail");

        match err {
            XurlError::SkillSelectionRequired { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
                assert!(candidates[0].starts_with("skills://github.com/Xuanwo/xurl/"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    fn create_git_remote(base: &Path, owner: &str, repo: &str, files: &[(char, &str, &str)]) {
        let work = base.join("work");
        fs::create_dir_all(&work).expect("mkdir work");
        run_git(["init", work.to_str().expect("path")], base);

        for (_, relative, content) in files {
            let path = work.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir file parent");
            }
            fs::write(path, content).expect("write file");
        }

        run_git(["-C", work.to_str().expect("path"), "add", "."], base);
        run_git(
            [
                "-C",
                work.to_str().expect("path"),
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "init",
            ],
            base,
        );

        let bare = base.join(owner).join(format!("{repo}.git"));
        if let Some(parent) = bare.parent() {
            fs::create_dir_all(parent).expect("mkdir bare parent");
        }
        run_git(
            [
                "clone",
                "--bare",
                work.to_str().expect("path"),
                bare.to_str().expect("path"),
            ],
            base,
        );
    }

    fn run_git<const N: usize>(args: [&str; N], cwd: &Path) {
        let output = std::process::Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("git command should run");
        if !output.status.success() {
            panic!(
                "git command failed: {}\nstdout={}\nstderr={}",
                args.join(" "),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }
}
