# git-perf Documentation

Welcome to the git-perf documentation! This page serves as your guide to all available documentation.

## Getting Started

New to git-perf? Start here:

- **[Quick Start](../README.md#quick-start)** - Get running in 5 minutes with basic commands
- **[Installation Guide](../README.md#installation)** - Multiple installation methods (shell installer, crates.io, pre-built binaries, from source)
- **[Integration Tutorial](./INTEGRATION_TUTORIAL.md)** - Complete step-by-step guide for GitHub Actions setup

## User Guides

Detailed guides for specific use cases:

- **[Configuration Guide](../README.md#configuration)** - Complete `.gitperfconfig` reference
  - Statistical dispersion methods (stddev vs MAD)
  - Per-measurement configuration overrides
  - Epoch management
  - Unit configuration
- **[Audit System](../README.md#audit-system)** - Understanding regression detection
  - Statistical analysis methods
  - Threshold configuration
  - Interpreting audit output
  - Managing epochs for expected changes
- **[Importing Measurements](./importing-measurements.md)** - Import test execution times and benchmark results
  - JUnit XML format (pytest, Jest, cargo-nextest, JUnit, and more)
  - Criterion JSON format (Rust benchmarks)
  - Cross-language examples and best practices
- **[Non-HEAD Measurements](./non-head-measurements.md)** - Add measurements to specific commits
  - Using `--commit` flag for write operations
  - Targeting specific commits, branches, and tags
  - Historical data backfilling
  - Branch comparison without switching
  - Technical details and best practices

## Reference Documentation

Technical reference material:

- **[CLI Reference](./manpage.md)** - Complete command-line reference for all git-perf commands
  - All subcommands (measure, add, import, audit, report, etc.)
  - Command options and flags
  - Usage examples
- **[Configuration File Reference](./example_config.toml)** - Annotated `.gitperfconfig` template
  - All available configuration options
  - Example values with explanations
  - Per-measurement overrides
- **[FAQ](../README.md#frequently-asked-questions)** - Frequently asked questions
  - General usage
  - Configuration and units
  - Audit and regression detection
  - Data management
  - GitHub Actions integration
  - Troubleshooting

## Contributing

Resources for contributors and developers:

- **[Contributing Guide](../CONTRIBUTING.md)** - How to contribute to git-perf
  - Code of conduct
  - Development workflow
  - Code quality standards
  - Testing requirements
  - Pull request guidelines
  - Commit message format (Conventional Commits)
- **[Development Setup](../CLAUDE.md)** - Developer and AI agent instructions
  - Quick reference commands
  - Pull request requirements
  - Testing setup
  - Documentation generation
  - Project architecture
- **[Release Process](../RELEASE.md)** - How releases are automated
  - release-plz and cargo-dist workflow
  - Release flow diagram
  - Environment setup

## Additional Resources

- **[Evaluation Tools](../evaluation/INDEX.md)** - Statistical method comparison tools
  - Comparing stddev vs MAD dispersion methods
  - Evaluation scripts and results
- **[Live Example Report](https://kaihowl.github.io/git-perf/master.html)** - See git-perf in action
- **[GitHub Discussions](https://github.com/kaihowl/git-perf/discussions)** - Ask questions and share ideas
- **[GitHub Issues](https://github.com/kaihowl/git-perf/issues)** - Report bugs or request features

## GitHub Actions

Reusable GitHub Actions for CI/CD integration:

- **[Install Action](../.github/actions/install/)** - Install git-perf in workflows
- **[Report Action](../.github/actions/report/)** - Generate and publish performance reports
  - Automatic PR comments with results
  - GitHub Pages integration
  - Audit integration
- **[Cleanup Action](../.github/actions/cleanup/)** - Remove old measurements and reports
  - Configurable retention periods
  - Dry-run support

## Documentation Organization

```
docs/
├── README.md (this file)                  # Documentation index
├── INTEGRATION_TUTORIAL.md                # End-to-end GitHub Actions setup
├── importing-measurements.md              # Test and benchmark import guide
├── non-head-measurements.md               # Guide for adding measurements to specific commits
├── manpage.md                             # CLI reference (auto-generated)
├── example_config.toml                    # Configuration template
└── plans/                                 # Feature design documents (internal)
```

## How git-perf Works

Want to understand the internals?

- **[Storage Model](../README.md#how-git-perf-works-storage-and-workflow)** - How measurements are stored using git-notes
- **[Merge Strategy](../README.md#how-git-perf-works-storage-and-workflow)** - How concurrent measurements are handled
- **[Pull Request Workflow](../README.md#how-git-perf-works-storage-and-workflow)** - How PR measurements work with first-parent traversal

## Need Help?

- **FAQ**: Check the [Frequently Asked Questions](../README.md#frequently-asked-questions) for common issues
- **Troubleshooting**: See [Integration Tutorial Troubleshooting](./INTEGRATION_TUTORIAL.md#troubleshooting) for CI/CD issues
- **Issues**: [Open an issue](https://github.com/kaihowl/git-perf/issues/new/choose) if you can't find an answer
- **Discussions**: [Start a discussion](https://github.com/kaihowl/git-perf/discussions) for questions or ideas

---

**Documentation Version**: Matches git-perf main branch
**Last Updated**: Auto-updated with each documentation change
