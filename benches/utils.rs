use std::{env, process::Command};

pub fn hermetic_git_env() {
    env::set_var("GIT_CONFIG_NOSYSTEM", "true");
    env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
    env::set_var("GIT_AUTHOR_NAME", "testuser");
    env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
    env::set_var("GIT_COMMITTER_NAME", "testuser");
    env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");
}

pub fn init_repo() {
    assert!(Command::new("git")
        .arg("init")
        .output()
        .expect("Failed to init git repo")
        .status
        .success());
}

pub fn empty_commit() {
    assert!(Command::new("git")
        .args(["commit", "--allow-empty", "-m", "test commit"])
        .output()
        .expect("Failed to init repo")
        .status
        .success());
}
