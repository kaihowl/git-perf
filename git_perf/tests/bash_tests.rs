#[cfg(test)]
mod test {
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use rstest::*;

    fn run_bash_test(bash_file: &Path) {
        let binary_path = Path::new(env!("CARGO_BIN_EXE_git-perf"));

        // Prepend binary path to PATH
        let mut paths =
            env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
        paths.insert(0, binary_path.parent().unwrap().to_path_buf());
        let new_path = env::join_paths(paths).expect("Failed to join PATH");

        // Run the bash test script with the updated PATH and hermetic git environment
        // These environment variables ensure tests don't use system/global git config
        // which could have commit signing or other settings that interfere with tests
        let output = Command::new("bash")
            .args([bash_file])
            .env("PATH", new_path)
            .env("GIT_CONFIG_NOSYSTEM", "true")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "github-actions[bot]")
            .env(
                "GIT_AUTHOR_EMAIL",
                "41898282+github-actions[bot]@users.noreply.github.com",
            )
            .env("GIT_COMMITTER_NAME", "github-actions[bot]")
            .env(
                "GIT_COMMITTER_EMAIL",
                "41898282+github-actions[bot]@users.noreply.github.com",
            )
            .output()
            .expect("Failed to run bash test");

        println!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.status.success(), "Bash test script failed");
    }

    #[rstest]
    fn for_each_file(#[files("../test/test*.sh")] path: PathBuf) {
        run_bash_test(&path);
    }
}
