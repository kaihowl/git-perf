]633;E;for file in target/man/man1/git-perf.1 target/man/man1/git-perf-add.1 target/man/man1/git-perf-audit.1 target/man/man1/git-perf-bump-epoch.1 target/man/man1/git-perf-measure.1 target/man/man1/git-perf-prune.1 target/man/man1/git-perf-pull.1 target/man/man1/git-perf-push.1 target/man/man1/git-perf-remove.1 target/man/man1/git-perf-report.1\x3b do     echo "$(basename "$file" .1)"\x3b     echo "================"\x3b     pandoc -f man -t gfm "$file"\x3b     echo -e "\\n\\n"\x3b done > docs/manpage.md;347d5795-595c-416b-a14d-3b2e3f2b5308]633;Cgit-perf
================



git-perf-add
================



git-perf-audit
================



git-perf-bump-epoch
================



git-perf-measure
================



git-perf-prune
================



git-perf-pull
================



git-perf-push
================



git-perf-remove
================



git-perf-report
================



