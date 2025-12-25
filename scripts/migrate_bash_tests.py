#!/usr/bin/env python3
"""
Bash Test Migration Script

Migrates bash tests from old pattern-based approach to new testing framework.
Based on docs/TESTING_MIGRATION.md
"""

import re
import sys
import shutil
import difflib
import argparse
from pathlib import Path
from typing import List, Dict, Optional, Tuple


class ExecutionContext:
    """Tracks whether we're in setup or test phase"""
    SETUP = "setup"
    TEST = "test"

    def __init__(self):
        self.current = self.SETUP
        self.in_section = False

    def analyze_line(self, line: str) -> str:
        """Determine if this line is setup or test context"""
        stripped = line.strip()

        # Test context markers
        if 'test_section' in stripped:
            self.current = self.TEST
            self.in_section = True
            return self.current

        # Echo statements that look like test descriptions (start with capital)
        if re.match(r'^echo ["\'][A-Z][^"\']*["\']$', stripped):
            self.current = self.TEST
            self.in_section = True
            return self.current

        # Commands with && exit 1 are always tests
        if '&& exit 1' in stripped:
            return self.TEST

        return self.current


class MigrationRiskAnalyzer:
    """Identifies patterns that are risky to auto-migrate"""

    RISKY_PATTERNS = [
        (r'\$\([^)]*\([^)]*\)[^)]*\)', 'Nested command substitution'),
        (r'if.*&&.*\|\|', 'Complex conditional with && and ||'),
        (r'(for|while).*exit', 'Loop with exit'),
        (r'[0-9]+>&[0-9]+.*2>&1', 'Complex stderr/stdout redirection'),
    ]

    def analyze_file(self, lines: List[str]) -> List[Dict]:
        """Returns list of risky lines with explanations"""
        risks = []

        for i, line in enumerate(lines, 1):
            for pattern, reason in self.RISKY_PATTERNS:
                if re.search(pattern, line):
                    risks.append({
                        'line': i,
                        'content': line.strip(),
                        'reason': reason
                    })

        return risks


class MigrationValidator:
    """Validates migrated tests for correctness"""

    VALID_ASSERTIONS = [
        'assert_equals', 'assert_not_equals',
        'assert_contains', 'assert_not_contains',
        'assert_matches', 'assert_not_matches',
        'assert_success', 'assert_success_with_output',
        'assert_failure', 'assert_failure_with_output',
        'assert_true', 'assert_false',
        'assert_file_exists', 'assert_file_not_exists',
        'assert_dir_exists',
        'assert_output_contains', 'assert_output_not_contains',  # Old style, still valid
    ]

    def validate(self, original_lines: List[str], transformed_lines: List[str]) -> List[str]:
        """Run validation checks"""
        issues = []

        # Check 1: No excessive loss of commands
        orig_commands = self._count_commands(original_lines)
        trans_commands = self._count_commands(transformed_lines)

        if trans_commands < orig_commands * 0.85:  # Allow 15% reduction
            issues.append(f"Warning: Command count reduced from {orig_commands} to {trans_commands}")

        # Check 2: All assert_ calls are valid
        for i, line in enumerate(transformed_lines, 1):
            if 'assert_' in line and not line.strip().startswith('#'):
                if not self._is_valid_assertion(line):
                    issues.append(f"Line {i}: Possibly invalid assertion syntax")

        # Check 3: Balanced test_section and test_stats
        has_section = any('test_section' in l for l in transformed_lines)
        has_stats = any('test_stats' in l for l in transformed_lines)

        if has_section and not has_stats:
            issues.append("Warning: test_section found but no test_stats")

        return issues

    def _count_commands(self, lines: List[str]) -> int:
        """Count substantial bash commands"""
        count = 0
        for line in lines:
            stripped = line.strip()
            if (stripped and
                not stripped.startswith('#') and
                stripped not in ['fi', 'done', 'esac', 'then', 'else', 'elif']):
                count += 1
        return count

    def _is_valid_assertion(self, line: str) -> bool:
        """Check if assertion has valid syntax"""
        return any(assertion in line for assertion in self.VALID_ASSERTIONS)


def transform_header(lines: List[str]) -> List[str]:
    """Pass 1: Transform header directives"""
    result = []
    i = 0
    added_test_trace = False

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # Check for set -e and set -x combination
        if (i + 1 < len(lines) and
            stripped == 'set -e' and
            lines[i+1].strip() == 'set -x'):
            if not added_test_trace:
                result.append('# Disable verbose tracing for cleaner output\n')
                result.append('export TEST_TRACE=0\n')
                added_test_trace = True
            i += 2
            continue

        # Check for set -euxo pipefail or similar
        if re.match(r'^set -(e|x|euxo)', stripped):
            if not added_test_trace and stripped != 'set -e':
                result.append('# Disable verbose tracing for cleaner output\n')
                result.append('export TEST_TRACE=0\n')
                added_test_trace = True
            i += 1
            continue

        result.append(line)
        i += 1

    return result


def transform_command_execution(lines: List[str]) -> List[str]:
    """Pass 2: Transform command execution patterns"""
    result = []
    context = ExecutionContext()

    for line in lines:
        current_context = context.analyze_line(line)
        stripped = line.strip()
        indent_match = re.match(r'^(\s*)', line)
        indent = indent_match.group(1) if indent_match else ''

        transformed = False

        # Pattern 1 (most specific): output=$(cmd 2>&1 1>/dev/null) && exit 1
        match = re.match(r'^(\s*)(\w+)=\$\(([^)]+) 2>&1 1>/dev/null\) && exit 1$', line)
        if match:
            indent = match.group(1)
            var = match.group(2)
            cmd = match.group(3)
            result.append(f"{indent}assert_failure_with_output {var} {cmd}\n")
            transformed = True

        # Pattern 2: output=$(cmd 2>&1 || true)
        if not transformed:
            match = re.match(r'^(\s*)(\w+)=\$\(([^)]+) 2>&1 \|\| true\)$', line)
            if match:
                indent = match.group(1)
                var = match.group(2)
                cmd = match.group(3)
                result.append(f"{indent}assert_failure_with_output {var} {cmd}\n")
                transformed = True

        # Pattern 3: Simple cmd && exit 1 (must be after output capture patterns)
        if not transformed:
            match = re.match(r'^(\s*)(\S.*?) && exit 1$', line)
            if match:
                indent = match.group(1)
                cmd = match.group(2)
                result.append(f"{indent}assert_failure {cmd}\n")
                transformed = True

        # Pattern 4: Simple output=$(cmd) in TEST context
        # Be conservative - only transform if clearly in test context
        if not transformed and current_context == ExecutionContext.TEST:
            match = re.match(r'^(\s*)output=\$\(([^)]+)\)$', line)
            if match and '2>&1' not in line:
                # Check next few lines for assert_output_contains or grep
                # This is a heuristic - only transform if output is used for assertions
                indent = match.group(1)
                cmd = match.group(2)
                # For now, keep conservative and don't transform - too risky
                # result.append(f"{indent}assert_success_with_output output {cmd}\n")
                # transformed = True

        if not transformed:
            result.append(line)

    return result


def transform_output_validation(lines: List[str]) -> List[str]:
    """Pass 3: Transform output validation patterns"""
    result = []

    for line in lines:
        new_line = line

        # Pattern 1: assert_output_contains -> assert_contains
        new_line = re.sub(
            r'\bassert_output_contains\b',
            'assert_contains',
            new_line
        )

        # Pattern 2: assert_output_not_contains -> assert_not_contains
        new_line = re.sub(
            r'\bassert_output_not_contains\b',
            'assert_not_contains',
            new_line
        )

        # Pattern 3: echo "$output" | grep -q "pattern"
        match = re.match(r'^(\s*)echo "(\$\w+)" \| grep -q "([^"]+)"$', new_line)
        if match:
            indent = match.group(1)
            var = match.group(2)
            pattern = match.group(3)
            new_line = f'{indent}assert_contains "{var}" "{pattern}"\n'

        # Pattern 4: ! echo "$output" | grep -q "pattern"
        match = re.match(r'^(\s*)! echo "(\$\w+)" \| grep -q "([^"]+)"$', new_line)
        if match:
            indent = match.group(1)
            var = match.group(2)
            pattern = match.group(3)
            new_line = f'{indent}assert_not_contains "{var}" "{pattern}"\n'

        result.append(new_line)

    return result


def transform_multiline_patterns(lines: List[str]) -> List[str]:
    """Pass 4: Transform multi-line patterns (if/then/fi blocks)"""
    result = []
    i = 0

    while i < len(lines):
        line = lines[i]

        # Check for if statement
        if line.strip().startswith('if'):
            if_block, end_idx = _parse_if_statement(lines, i)

            # Try transformations
            transformed = (_transform_if_grep_exit(if_block) or
                          _transform_if_test_exit(if_block) or
                          _transform_if_regex_exit(if_block))

            if transformed:
                result.append(transformed)
                i = end_idx + 1
                continue

        result.append(line)
        i += 1

    return result


def _parse_if_statement(lines: List[str], start_idx: int) -> Tuple[List[str], int]:
    """Parse if...fi block and return (block_lines, end_index)"""
    depth = 0
    end_idx = start_idx

    for i in range(start_idx, len(lines)):
        stripped = lines[i].strip()
        if stripped.startswith('if '):
            depth += 1
        elif stripped == 'fi':
            depth -= 1
            if depth == 0:
                end_idx = i
                break

    return lines[start_idx:end_idx+1], end_idx


def _transform_if_grep_exit(if_block: List[str]) -> Optional[str]:
    """Transform: if ! echo "$var" | grep -q "pattern"; then ... exit 1; fi"""
    if len(if_block) < 3:
        return None

    first_line = if_block[0]

    # Pattern: if ! echo "$var" | grep -q "pattern"; then
    match = re.match(r'^(\s*)if ! echo "(\$\w+)" \| grep -q "([^"]+)"; then', first_line)

    if not match:
        return None

    indent = match.group(1)
    var = match.group(2)
    pattern = match.group(3)

    # Check if body contains exit 1
    body = ''.join(if_block[1:-1])
    if 'exit 1' in body:
        return f'{indent}assert_contains "{var}" "{pattern}"\n'

    return None


def _transform_if_test_exit(if_block: List[str]) -> Optional[str]:
    """Transform: if [[ -z "$output" ]]; then ... exit 1; fi"""
    if len(if_block) < 3:
        return None

    first_line = if_block[0]

    # Pattern: if [[ -z "$var" ]]; then
    match = re.match(r'^(\s*)if \[\[ -z "(\$\w+)" \]\]; then', first_line)
    if match:
        body = ''.join(if_block[1:-1])
        if 'exit 1' in body:
            indent = match.group(1)
            var = match.group(2)
            return f'{indent}assert_not_equals "{var}" ""\n'

    # Pattern: if [[ -n "$var" ]]; then (negated - looking for non-empty when we expect empty)
    match = re.match(r'^(\s*)if \[\[ -n "(\$\w+)" \]\]; then', first_line)
    if match:
        body = ''.join(if_block[1:-1])
        if 'exit 1' in body:
            indent = match.group(1)
            var = match.group(2)
            return f'{indent}assert_equals "{var}" ""\n'

    return None


def _transform_if_regex_exit(if_block: List[str]) -> Optional[str]:
    """Transform: if ! [[ "$output" =~ $regex ]]; then ... exit 1; fi"""
    if len(if_block) < 3:
        return None

    first_line = if_block[0]

    # Pattern: if ! [[ ${var} =~ pattern ]]; then
    match = re.match(r'^(\s*)if ! \[\[ \$\{?(\w+)\}? =~ (.+) \]\]; then', first_line)
    if match:
        body = ''.join(if_block[1:-1])
        if 'exit 1' in body:
            indent = match.group(1)
            var = match.group(2)
            pattern = match.group(3).strip()
            return f'{indent}assert_matches "${var}" "{pattern}"\n'

    # Pattern: if [[ ! ${var} =~ pattern ]]; then
    match = re.match(r'^(\s*)if \[\[ ! \$\{?(\w+)\}? =~ (.+) \]\]; then', first_line)
    if match:
        body = ''.join(if_block[1:-1])
        if 'exit 1' in body:
            indent = match.group(1)
            var = match.group(2)
            pattern = match.group(3).strip()
            return f'{indent}assert_matches "${var}" "{pattern}"\n'

    return None


def transform_section_markers(lines: List[str]) -> List[str]:
    """Pass 5: Convert echo statements to test_section markers"""
    result = []

    for line in lines:
        # Check for echo "Description" pattern - must start with capital or number
        match = re.match(r'^(\s*)echo (["\'])([A-Z0-9][^"\']*)\2$', line)

        if match:
            indent = match.group(1)
            description = match.group(3)
            result.append(f'{indent}test_section "{description}"\n')
        else:
            result.append(line)

    return result


def add_final_cleanup(lines: List[str]) -> List[str]:
    """Pass 6: Add test_stats before final exit 0"""
    result = list(lines)

    # Find last exit 0
    for i in range(len(result) - 1, -1, -1):
        if result[i].strip() == 'exit 0':
            # Check if test_stats already present
            has_test_stats = any('test_stats' in l for l in result[:i])

            if not has_test_stats:
                # Insert blank line and test_stats
                result.insert(i, 'test_stats\n')
            break

    return result


class BashTestMigrator:
    """Main migration orchestrator"""

    def __init__(self, dry_run: bool = True, backup: bool = True):
        self.dry_run = dry_run
        self.backup = backup
        self.stats = {
            'files_analyzed': 0,
            'files_migrated': 0,
            'files_skipped': 0,
            'risks_identified': 0
        }

    def migrate_file(self, filepath: Path) -> None:
        """Migrate a single test file"""
        print(f"\n{'='*70}")
        print(f"Migrating: {filepath}")
        print('='*70)

        # Read original
        with open(filepath, 'r') as f:
            original_lines = f.readlines()

        # Check if already migrated
        if any('export TEST_TRACE=' in l or 'TEST_TRACE=0' in l for l in original_lines):
            print("✓ Already migrated (has TEST_TRACE)")
            self.stats['files_skipped'] += 1
            return

        # Analyze risks
        risk_analyzer = MigrationRiskAnalyzer()
        risks = risk_analyzer.analyze_file(original_lines)

        if risks and len(risks) > 10:
            print(f"\n⚠️  Identified {len(risks)} risky patterns - too many for auto-migration")
            for risk in risks[:5]:
                print(f"  Line {risk['line']}: {risk['content'][:60]}")
                print(f"    → {risk['reason']}")
            print("  ... Manual review strongly recommended.")
            self.stats['risks_identified'] += len(risks)
            self.stats['files_skipped'] += 1
            return

        if risks:
            print(f"\n⚠️  Identified {len(risks)} risky pattern(s):")
            for risk in risks:
                print(f"  Line {risk['line']}: {risk['content'][:60]}")
                print(f"    → {risk['reason']}")

        # Apply transformations
        lines = original_lines

        print("\nApplying transformations:")
        print("  Pass 1: Headers...")
        lines = transform_header(lines)

        print("  Pass 2: Command execution...")
        lines = transform_command_execution(lines)

        print("  Pass 3: Output validation...")
        lines = transform_output_validation(lines)

        print("  Pass 4: Multi-line patterns...")
        lines = transform_multiline_patterns(lines)

        print("  Pass 5: Section markers...")
        lines = transform_section_markers(lines)

        print("  Pass 6: Final cleanup...")
        lines = add_final_cleanup(lines)

        # Validate
        validator = MigrationValidator()
        issues = validator.validate(original_lines, lines)

        if issues:
            print(f"\n⚠️  Validation issues:")
            for issue in issues:
                print(f"  - {issue}")

        # Show diff
        self._show_diff(original_lines, lines, filepath)

        # Write result
        if not self.dry_run:
            if self.backup:
                backup_path = str(filepath) + '.bak'
                shutil.copy2(filepath, backup_path)
                print(f"\n📁 Backup created: {backup_path}")

            with open(filepath, 'w') as f:
                f.writelines(lines)
            print(f"✓ File migrated successfully")
            self.stats['files_migrated'] += 1
        else:
            print("\n🔍 DRY RUN - No changes written")

    def _show_diff(self, original: List[str], transformed: List[str], filepath: Path) -> None:
        """Show unified diff of changes"""
        diff = difflib.unified_diff(
            original,
            transformed,
            fromfile=f"{filepath.name} (original)",
            tofile=f"{filepath.name} (migrated)",
            lineterm=''
        )

        diff_lines = [line for line in diff]

        if not diff_lines:
            print("\n📝 No changes made")
            return

        print("\n📝 Changes preview:")
        if len(diff_lines) <= 60:
            for line in diff_lines:
                print(line)
        else:
            for line in diff_lines[:30]:
                print(line)
            print(f"\n... ({len(diff_lines) - 60} lines omitted) ...\n")
            for line in diff_lines[-30:]:
                print(line)

    def migrate_directory(self, test_dir: Path) -> None:
        """Migrate all test files in directory"""
        test_files = sorted(test_dir.glob('test_*.sh'))

        # Filter out already migrated files
        old_style_files = []
        for f in test_files:
            with open(f, 'r') as file:
                content = file.read()
                if 'export TEST_TRACE=' not in content and 'TEST_TRACE=0' not in content:
                    old_style_files.append(f)

        print(f"Found {len(old_style_files)} old-style test files to migrate")
        print(f"(Out of {len(test_files)} total test files)")

        for filepath in old_style_files:
            try:
                self.migrate_file(filepath)
                self.stats['files_analyzed'] += 1
            except Exception as e:
                print(f"❌ Error migrating {filepath}: {e}")
                import traceback
                traceback.print_exc()

        self._print_summary()

    def _print_summary(self) -> None:
        """Print migration summary"""
        print(f"\n{'='*70}")
        print("MIGRATION SUMMARY")
        print('='*70)
        print(f"Files analyzed:       {self.stats['files_analyzed']}")
        print(f"Files migrated:       {self.stats['files_migrated']}")
        print(f"Files skipped:        {self.stats['files_skipped']}")
        print(f"Risks identified:     {self.stats['risks_identified']}")
        print('='*70)


def main():
    parser = argparse.ArgumentParser(
        description='Migrate bash tests from old pattern to new framework',
        epilog='Examples:\n'
               '  %(prog)s test/test_version.sh              # Dry-run single file\n'
               '  %(prog)s test/                             # Dry-run directory\n'
               '  %(prog)s test/ --no-dry-run                # Actually migrate\n'
               '  %(prog)s test/ --no-dry-run --no-backup    # Migrate without backups\n',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        'path',
        help='Test file or directory to migrate'
    )
    parser.add_argument(
        '--no-dry-run',
        action='store_true',
        help='Actually write changes (default is dry-run)'
    )
    parser.add_argument(
        '--no-backup',
        action='store_true',
        help='Do not create .bak backup files'
    )

    args = parser.parse_args()

    migrator = BashTestMigrator(
        dry_run=not args.no_dry_run,
        backup=not args.no_backup
    )

    path = Path(args.path)
    if not path.exists():
        print(f"Error: Path '{path}' does not exist")
        sys.exit(1)

    if path.is_dir():
        migrator.migrate_directory(path)
    else:
        migrator.migrate_file(path)
        migrator._print_summary()


if __name__ == '__main__':
    main()
