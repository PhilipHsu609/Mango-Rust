# Test Validation Framework

A mutation testing framework that validates test effectiveness by injecting bugs and verifying tests detect them.

## Overview

This framework uses **bug injection** (mutation testing) to validate that your integration tests are effective at catching bugs. It works by:

1. **Injecting bugs** into source code via temporary git branches
2. **Running tests** against the buggy code
3. **Determining test strength**:
   - **STRONG**: Test failed when bug was injected ✅ (good - test caught the bug)
   - **WEAK**: Test passed despite bug injection ❌ (bad - test missed the bug)
4. **Generating reports** with recommendations for improving weak tests
5. **Cleaning up** branches automatically

## Installation

```bash
cd tests/validation
npm install
```

## Quick Start

```bash
# Run full validation on all tests
npm run validate

# See what would be validated without running
npm run validate:dry-run

# Run with verbose output
npm run validate:verbose

# Get help
npm run help
```

## Usage

### Validate All Tests

```bash
npm run validate
```

This will:
- Load the bug catalog (`bug-catalog.yaml`)
- For each test and bug:
  - Create a temporary git branch
  - Inject the bug into source code
  - Run the test
  - Determine if test is STRONG or WEAK
  - Clean up the branch
- Generate comprehensive reports in `.validation-results/run-TIMESTAMP/`

**Output:**
- `results.json` - Full validation results (machine-readable)
- `report.md` - Markdown report with recommendations (human-readable)
- `report.json` - Structured report data

### Dry Run

Preview what will be validated without actually running:

```bash
npm run validate:dry-run
```

Shows:
- Number of tests
- Number of bug injections
- First 5 tests with their bugs

### Verbose Mode

See detailed output for debugging:

```bash
npm run validate:verbose
```

Shows:
- File paths being used
- Detailed error messages
- Test output
- Stack traces on errors

### Generate Report from Existing Results

If you have previous validation results:

```bash
npm run validate:report -- --report .validation-results/run-2025-01-15T10-30-00/results.json
```

This regenerates markdown and JSON reports without re-running validation.

## Command-Line Options

```
Options:
  -t, --test <id>       Validate specific test by ID
  -c, --catalog <file>  Bug catalog YAML file (default: bug-catalog.yaml)
  -o, --output <dir>    Output directory (default: .validation-results)
  -r, --report <file>   Existing results JSON for report generation
  -d, --dry-run         Show what would be validated
  -v, --verbose         Show detailed output
  --timeout <ms>        Test timeout in milliseconds (default: 60000)
  -h, --help            Show help
```

## Bug Catalog Format

The bug catalog (`bug-catalog.yaml`) defines which bugs to inject for each test.

### Structure

```yaml
version: "1.0"
generatedAt: "2025-01-15T10:00:00Z"

tests:
  - testId: "tests/integration/theme.spec.ts:10:5"
    testName: "Theme toggle should switch between light and dark"
    testFile: "tests/integration/theme.spec.ts"
    functionality: "Theme toggling and persistence"

    bugs:
      - id: "theme-toggle-1"
        description: "Break theme class application"
        targetFile: "static/js/core.js"
        strategy: "replace"
        find: "body.classList.add(theme === 'dark' ? 'uk-dark' : 'uk-light');"
        replace: "// Disabled theme class"
        expectedFailure: "Theme class not applied to body"

      - id: "theme-toggle-2"
        description: "Break localStorage persistence"
        targetFile: "static/js/core.js"
        strategy: "delete"
        find: "localStorage.setItem('theme', theme);"
        expectedFailure: "Theme not persisted across page loads"
```

### Fields

#### Test Entry

- `testId`: Unique test identifier (file:line:column format)
- `testName`: Human-readable test name
- `testFile`: Path to test file
- `functionality`: What the test validates
- `bugs`: Array of bug definitions

#### Bug Definition

- `id`: Unique bug identifier
- `description`: What this bug breaks
- `targetFile`: File to inject bug into (relative to project root)
- `strategy`: How to inject the bug
  - `replace`: Find exact string and replace with new string
  - `delete`: Remove exact string
  - `comment`: Comment out matching code
- `find`: Exact string to find in target file (must be unique)
- `replace`: Replacement string (required for `replace` strategy)
- `expectedFailure`: Hint about which assertion should fail

### Bug Injection Strategies

#### Replace Strategy

Finds exact string and replaces it:

```yaml
strategy: "replace"
find: "element.classList.add('visible');"
replace: "element.classList.add('hidden');"
```

#### Delete Strategy

Removes exact string:

```yaml
strategy: "delete"
find: "validateInput(value);"
# No replace field needed
```

#### Comment Strategy

Comments out code (uses `//` for JS/TS/Rust, `<!-- -->` for HTML):

```yaml
strategy: "comment"
find: "saveToDatabase(data);"
# Results in: // saveToDatabase(data);
```

## Understanding Results

### Validation Status

- **STRONG** ✅: Test failed when bug was injected (good - test is effective)
- **WEAK** ❌: Test passed despite bug (bad - test missed the bug)
- **INJECTION_FAILED** ⚠️: Bug couldn't be injected (pattern not found)
- **TIMEOUT** ⏱️: Test exceeded timeout
- **CRASH** 💥: Test crashed or threw unexpected error

### Exit Codes

- `0`: All tests are STRONG (success)
- `1`: Found weak tests or errors (failure)

This allows integration with CI/CD:

```bash
npm run validate || echo "Found weak tests!"
```

### Report Structure

The markdown report includes:

1. **Summary**: Statistics and coverage percentage
2. **Weak Tests**: Detailed analysis with:
   - What bug was missed
   - Why the test is weak
   - Specific recommendations
   - Code snippets showing fixes
3. **Strong Tests**: List of effective tests
4. **Best Practices**: Common patterns and guidelines

## Workflow

### Normal Workflow

```bash
# 1. Run validation
cd tests/validation
npm run validate

# 2. Review report
cat .validation-results/run-*/report.md

# 3. Fix weak tests based on recommendations

# 4. Re-run validation to verify fixes
npm run validate
```

### CI/CD Integration

```yaml
# .github/workflows/test-validation.yml
name: Test Validation

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install dependencies
        run: |
          cd tests
          npm install
          cd validation
          npm install

      - name: Run validation
        run: |
          cd tests/validation
          npm run validate

      - name: Upload reports
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: validation-reports
          path: tests/validation/.validation-results/
```

## Troubleshooting

### "Repository has uncommitted changes"

**Problem:** Validation requires a clean git repository.

**Solution:**
```bash
# Commit your changes
git add .
git commit -m "Your changes"

# Or stash them
git stash
```

### "Catalog file not found"

**Problem:** Bug catalog not found at expected location.

**Solution:**
```bash
# Ensure you're in the validation directory
cd tests/validation

# Or specify catalog path
npm run validate -- --catalog path/to/bug-catalog.yaml
```

### "Pattern not found in file"

**Problem:** Bug injection failed because the `find` pattern doesn't exist in the target file.

**Solution:**
- Verify the target file path is correct
- Check that the `find` string matches exactly (including whitespace)
- Ensure code hasn't changed since catalog was created
- Update bug catalog if code has been refactored

### "Pattern appears N times (must be unique)"

**Problem:** The `find` pattern appears multiple times in the file.

**Solution:**
- Make the `find` pattern more specific to match only one location
- Include surrounding context to make it unique

Example:
```yaml
# Instead of:
find: "element.click();"

# Use:
find: |
  const submitButton = element;
  element.click();
```

### "Playwright not available"

**Problem:** Playwright not installed in tests directory.

**Solution:**
```bash
cd ../  # Go to tests directory
npm install
npx playwright install
```

### Test Timeouts

**Problem:** Tests timing out (default 60s).

**Solution:**
```bash
# Increase timeout to 120 seconds
npm run validate -- --timeout 120000
```

### Validation Branches Not Cleaned Up

**Problem:** Previous validation run was interrupted, leaving branches.

**Solution:**
```bash
# View validation branches
git branch | grep test-validation

# Clean them up manually
git branch -D test-validation/theme-toggle/bug-1
# Or delete all at once
git branch | grep test-validation | xargs git branch -D
```

## Best Practices

### Writing Bug Catalog Entries

1. **One functionality per test**: Each test should validate one specific behavior
2. **Precise patterns**: Use unique `find` patterns with sufficient context
3. **Realistic bugs**: Inject bugs that could actually occur (typos, logic errors, missing code)
4. **Clear descriptions**: Describe what breaks, not how to fix it
5. **Expected failures**: Document which assertion should fail

### Interpreting Results

1. **Target 90%+ coverage**: Aim for ≥90% STRONG tests
2. **Fix weak tests**: Weak tests indicate missing assertions
3. **Review injection failures**: Update catalog if code changed
4. **Investigate timeouts**: May indicate infinite loops or performance issues

### Maintaining the Catalog

1. **Update after refactoring**: If code changes significantly, update patterns
2. **Add tests incrementally**: Start with critical paths, expand coverage
3. **Version control**: Commit bug catalog with tests
4. **Document assumptions**: Note any code patterns the catalog relies on

## Architecture

```
tests/validation/
├── cli.ts                    # CLI entry point
├── package.json              # Dependencies and scripts
├── tsconfig.json             # TypeScript configuration
├── bug-catalog.yaml          # Bug definitions
├── types/
│   └── index.ts             # TypeScript interfaces
├── utils/
│   └── git-utils.ts         # Git operations (branches, cleanup)
├── core/
│   ├── bug-injector.ts      # Bug injection logic
│   ├── test-runner.ts       # Playwright test execution
│   ├── orchestrator.ts      # Main workflow coordinator
│   └── report-generator.ts  # Report generation
└── .validation-results/     # Generated results (gitignored)
    └── run-TIMESTAMP/
        ├── results.json     # Full results
        ├── report.md        # Markdown report
        └── report.json      # Structured report
```

## Contributing

### Adding New Bug Patterns

1. Analyze test to understand what it validates
2. Identify code that, if broken, should make test fail
3. Create bug entry in `bug-catalog.yaml`
4. Run validation to verify test catches the bug
5. If test is WEAK, improve test assertions

### Improving Recommendations

Edit `core/report-generator.ts` to add pattern-based recommendations:

```typescript
if (bug.includes('your-pattern')) {
    return 'Your specific recommendation';
}
```

## FAQ

**Q: How long does validation take?**
A: Depends on test count and duration. With 50 tests and 63 bugs, expect 15-30 minutes.

**Q: Can I run validation in parallel?**
A: Not currently. Git branch operations must be sequential.

**Q: Does this modify my source code permanently?**
A: No. All changes are made in temporary git branches that are deleted after each test.

**Q: What if validation crashes?**
A: Branches should auto-cleanup, but verify with `git branch | grep test-validation`.

**Q: Can I validate a single test?**
A: Yes: `npm run validate -- --test "your-test-name"` (implementation pending).

**Q: How do I add more bugs to the catalog?**
A: Edit `bug-catalog.yaml` and add bug entries following the format above.

## License

MIT

## Support

For issues or questions:
1. Check troubleshooting section above
2. Review example bug catalog entries
3. Run with `--verbose` for detailed output
4. Check git repository is in clean state
