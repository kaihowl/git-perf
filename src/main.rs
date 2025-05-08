use anyhow::Result;
use git_perf::cli;

fn main() -> Result<()> {
    cli::handle_calls()
}

#[cfg(test)]
mod test {
    use std::env;
    use std::process::Command;

    use assert_cmd::cargo::cargo_bin;

    fn run_bash_tests(start_filter: &str) {
        let binary_path = cargo_bin("git-perf");

        // Prepend binary path to PATH
        let mut paths =
            env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
        paths.insert(0, binary_path.parent().unwrap().to_path_buf());
        let new_path = env::join_paths(paths).expect("Failed to join PATH");

        // Run the bash test script with the updated PATH
        let output = Command::new("bash")
            .args(["test/run_tests.sh", start_filter])
            .env("PATH", new_path)
            .output()
            .expect("Failed to run bash test");

        println!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.status.success(), "Bash test script failed");
    }

    #[test]
    fn run_quick_bash_tests_with_binary() {
        run_bash_tests("test_");
    }

    #[test]
    fn run_slow_bash_tests_with_binary() {
        run_bash_tests("testslow_");
    }
}
