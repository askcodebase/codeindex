use std::path::Path;

use codeindex_common::git2::{Cred, Repository};

fn clone_repo(url: &str) -> Result<(), git2::Error> {
    let creds = Cred::default();
    let repos_path = Path::new("repos");

    let repo_name = url
        .split('/')
        .last()
        .expect("Failed to parse repo name from URL");

    let repo_path = repos_path.join(repo_name);

    if repo_path.exists() {
        return Ok(()); // Repo already cloned
    }

    println!("Cloning {} to {}", url, repo_path.display());

    let repo = Repository::clone(url, &repo_path, Some(&creds))?;

    println!("Cloned {} successfully", repo.path().display());

    Ok(())
}
