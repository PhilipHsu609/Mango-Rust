# Tasks Document: Test Validation Framework

## Task Breakdown

### Phase 1: Bug Catalog Creation

- [x] 1.1. Analyze theme toggle tests and define bugs
  - Files: `tests/validation/bug-catalog.yaml` (start creating)
  - Analyze all 10 theme tests in theme.spec.ts
  - Define 1-2 bugs per test that would break specific assertions
  - Document expected test failures for each bug
  - Purpose: Create comprehensive bug catalog for theme functionality validation
  - _Leverage: tests/integration/theme.spec.ts, static/js/core.js_
  - _Requirements: REQ-1 (Bug Injection Catalog)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Test Analyst specializing in mutation testing and test analysis | Task: Analyze all 10 theme toggle tests in theme.spec.ts, identify specific functionality each test validates, and define 1-2 bugs per test that would break those specific assertions following requirement REQ-1 | Restrictions: Bugs must be minimal and targeted, only break the specific test being validated, must be reversible via git, document exact file path and line to modify | Success: Bug catalog entries created for all theme tests with clear bug descriptions, file paths, find/replace patterns, and expected failure messages documented in YAML format | Instructions: Before starting, mark this task as in-progress in tasks.md by changing `[ ]` to `[-]`. After completing analysis, use the log-implementation tool with comprehensive artifacts documenting bug catalog structure. Finally, mark task as complete `[x]` in tasks.md._

- [x] 1.2. Analyze reader tests and define bugs
  - Files: `tests/validation/bug-catalog.yaml` (continue)
  - Analyze all 10 reader tests in reader.spec.ts and reader-theme.spec.ts
  - Define bugs that break page navigation, mode switching, and settings
  - Target both JavaScript (reader.js) and Rust backend if applicable
  - Purpose: Create bug catalog entries for reader functionality
  - _Leverage: tests/integration/reader.spec.ts, templates/reader.js_
  - _Requirements: REQ-1 (Bug Injection Catalog)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Test Analyst with expertise in reader functionality testing | Task: Analyze all reader-related tests, identify functionality validated (page navigation, mode switching, settings), and define targeted bugs following requirement REQ-1 | Restrictions: Bugs must target specific reader features, be reversible, document exact code locations, distinguish between frontend JS and backend Rust bugs | Success: Bug catalog has complete entries for all reader tests with precise bug injection instructions for both JavaScript and Rust code where applicable | Instructions: Mark in-progress, analyze tests thoroughly, log implementation with bug catalog examples, mark complete._

- [x] 1.3. Analyze library and navigation tests and define bugs
  - Files: `tests/validation/bug-catalog.yaml` (continue)
  - Analyze 12 library tests and 12 navigation tests
  - Define bugs for search, sort, filtering, and navigation functionality
  - Target Alpine.js code, HTML templates, and Rust routes
  - Purpose: Complete bug catalog for all 50 tests
  - _Leverage: tests/integration/library.spec.ts, tests/integration/navigation.spec.ts_
  - _Requirements: REQ-1 (Bug Injection Catalog)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Full-stack QA Analyst | Task: Analyze remaining library and navigation tests, define bugs for search/sort/filter/navigation functionality covering all 50 tests following requirement REQ-1 | Restrictions: Bugs must cover frontend Alpine.js, templates, and backend routes, be minimal and reversible, document all injection targets precisely | Success: Bug catalog is 100% complete with entries for all 50 integration tests, properly formatted YAML with all required fields, ready for automation | Instructions: Mark in-progress, complete catalog systematically, log with full catalog statistics and examples, mark complete._

### Phase 2: Core Framework Implementation

- [x] 2.1. Set up validation project structure
  - Files: `tests/validation/package.json`, `tests/validation/tsconfig.json`, `tests/validation/.gitignore`
  - Create package.json with dependencies (simple-git, yaml, chalk, ora)
  - Configure TypeScript for validation framework
  - Add .gitignore for .validation-results/, node_modules/
  - Purpose: Initialize validation framework with proper tooling
  - _Leverage: tests/package.json (existing test project structure)_
  - _Requirements: REQ-2 (Automated Bug Injection), Non-functional (Code Architecture)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: DevOps Engineer specializing in TypeScript project setup | Task: Initialize validation framework project following requirements REQ-2 and architecture standards, installing simple-git for git operations, yaml for catalog parsing, chalk and ora for CLI output | Restrictions: Use latest stable versions, TypeScript strict mode, follow Node.js best practices, keep dependencies minimal | Success: package.json created with correct dependencies, tsconfig.json configured properly, project structure established, npm install succeeds | Instructions: Mark in-progress, set up project thoroughly, log with dependencies and configuration, mark complete._

- [x] 2.2. Create TypeScript interfaces and types
  - Files: `tests/validation/types/index.ts`
  - Define BugDefinition, BranchInfo, TestSpec, TestResult, ValidationResult, ValidationRun interfaces
  - Define BugStrategy type ('replace' | 'delete' | 'comment')
  - Define ValidationStatus type ('STRONG' | 'WEAK' | 'INJECTION_FAILED' | 'TIMEOUT' | 'CRASH')
  - Purpose: Establish type safety for validation framework
  - _Leverage: Design document interfaces_
  - _Requirements: Non-functional (Code Architecture)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: TypeScript Developer specializing in type systems | Task: Create comprehensive TypeScript interfaces for validation framework following design document specifications and architecture requirements | Restrictions: Use strict TypeScript types, make interfaces extensible, follow naming conventions, ensure type safety throughout | Success: All interfaces defined with proper types, enums for strategy and status, interfaces match design document exactly, well-documented with JSDoc | Instructions: Mark in-progress, define all types precisely, log with interface definitions, mark complete._

- [x] 2.3. Implement git utilities module
  - Files: `tests/validation/utils/git-utils.ts`
  - Implement getCurrentBranch(), createBranch(name), switchBranch(name), deleteBranch(name)
  - Implement checkCleanState() to verify no uncommitted changes
  - Add error handling for git failures with clear messages
  - Purpose: Provide reliable git operations for bug injection
  - _Leverage: simple-git npm package_
  - _Requirements: REQ-2 (Automated Bug Injection), REQ-4 (Branch Cleanup)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Git Operations Engineer | Task: Implement git utilities following requirements REQ-2 and REQ-4 using simple-git package, providing functions for branch creation, switching, and cleanup with robust error handling | Restrictions: Always check for clean state before operations, use descriptive branch names (test-validation/*), handle errors gracefully, never force operations | Success: All git functions work reliably, clean state verified before operations, branch operations are atomic and reversible, clear error messages | Instructions: Mark in-progress, implement with safety checks, log with function signatures and error handling, mark complete._

- [x] 2.4. Implement bug injector module
  - Files: `tests/validation/core/bug-injector.ts`
  - Implement BugInjector class with createBugBranch(), applyBugChange(), switchBack(), cleanup()
  - Support 'replace', 'delete', and 'comment' strategies
  - Implement exact string matching and replacement in files
  - Add validation that bug was actually applied
  - Purpose: Core functionality for injecting bugs into codebase
  - _Leverage: tests/validation/utils/git-utils.ts, fs/promises for file operations_
  - _Requirements: REQ-2 (Automated Bug Injection)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Code Mutation Engineer | Task: Implement BugInjector class following requirement REQ-2, supporting replace/delete/comment strategies with exact string matching and validation | Restrictions: Verify file exists before modification, validate exact string found, create unique branch names, confirm changes applied, handle encoding properly (UTF-8) | Success: BugInjector creates branches correctly, applies all three strategies accurately, validates changes, handles errors gracefully, leaves repo in clean state on failure | Instructions: Mark in-progress, implement carefully with validation, log with class methods and examples, mark complete._

- [x] 2.5. Implement test runner module
  - Files: `tests/validation/core/test-runner.ts`
  - Implement TestRunner class with runTest(spec) method
  - Execute Playwright tests via child_process with proper grep filtering
  - Parse test output to determine pass/fail status
  - Capture stdout, stderr, exit code, and duration
  - Add timeout handling (60 seconds max)
  - Purpose: Execute individual tests and capture results
  - _Leverage: child_process.spawn, existing Playwright test infrastructure_
  - _Requirements: REQ-3 (Test Execution and Validation)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Execution Engineer | Task: Implement TestRunner class following requirement REQ-3, executing Playwright tests via npx with proper filtering and result parsing | Restrictions: Use --grep to run single test, timeout at 60s, capture full output, parse exit code correctly (0=pass, non-zero=fail), handle test crashes | Success: TestRunner executes single tests reliably, correctly determines pass/fail, captures complete output, times out properly, handles crashes gracefully | Instructions: Mark in-progress, implement with robust parsing, log with execution examples, mark complete._

- [x] 2.6. Implement validation orchestrator
  - Files: `tests/validation/core/orchestrator.ts`
  - Implement ValidationOrchestrator class with validateAll() and validateTest() methods
  - Coordinate workflow: load catalog → inject bug → run test → validate → cleanup
  - Determine STRONG (test failed as expected) vs WEAK (test passed despite bug)
  - Implement progress tracking with ora spinners
  - Add comprehensive error handling and cleanup on failure
  - Purpose: Main orchestration logic for validation workflow
  - _Leverage: BugInjector, TestRunner, bug-catalog.yaml_
  - _Requirements: REQ-3 (Test Execution and Validation), REQ-4 (Branch Cleanup)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Validation Workflow Engineer | Task: Implement ValidationOrchestrator following requirements REQ-3 and REQ-4, coordinating full validation workflow with progress indicators and cleanup | Restrictions: Always clean up branches even on error, show progress for each test, validate test should FAIL when bug injected (FAIL=STRONG, PASS=WEAK), handle partial failures gracefully | Success: Orchestrator runs full validation workflow correctly, determines STRONG/WEAK accurately, shows clear progress, cleans up all branches, recovers from errors | Instructions: Mark in-progress, implement workflow carefully, log with workflow diagram and examples, mark complete._

### Phase 3: Reporting and CLI

- [x] 3.1. Implement report generator module
  - Files: `tests/validation/core/report-generator.ts`
  - Implement ReportGenerator class with generateMarkdownReport() and generateJSONReport()
  - Create markdown report with summary, weak tests section, strong tests section
  - Include recommendations for weak tests based on bug injected
  - Add timestamps, durations, and statistics
  - Purpose: Generate comprehensive human-readable and machine-readable reports
  - _Leverage: ValidationRun data structure_
  - _Requirements: REQ-5 (Validation Report Generation), REQ-6 (Test Improvement Recommendations)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Technical Writer and Report Engineer | Task: Implement ReportGenerator following requirements REQ-5 and REQ-6, creating markdown and JSON reports with statistics, weak test analysis, and recommendations | Restrictions: Follow markdown best practices, include code snippets for recommendations, format dates clearly, make reports actionable, JSON must be valid and structured | Success: Markdown reports are clear and actionable, weak tests have specific recommendations with code examples, JSON reports are valid and complete, reports are easy to read and understand | Instructions: Mark in-progress, implement with good formatting, log with report examples, mark complete._

- [x] 3.2. Implement CLI interface
  - Files: `tests/validation/cli.ts`
  - Create CLI entry point with argument parsing (--test, --file, --dry-run, --verbose)
  - Implement commands: validate all, validate single test, generate report
  - Add colored output with chalk for success/failure/warnings
  - Add verbose mode for debugging
  - Purpose: Provide user-friendly command-line interface
  - _Leverage: ValidationOrchestrator, ReportGenerator, chalk, ora_
  - _Requirements: All (CLI is primary interface)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CLI Developer specializing in Node.js command-line tools | Task: Create comprehensive CLI interface covering all requirements, supporting full validation, single test validation, and report generation with proper argument handling | Restrictions: Use minimist or yargs for argument parsing, provide help text, validate arguments, use colors appropriately (green=success, red=failure, yellow=warning), handle errors gracefully | Success: CLI provides all commands documented in design, help text is clear, colored output is readable, errors are handled well, dry-run shows what would happen | Instructions: Mark in-progress, implement user-friendly CLI, log with command examples, mark complete._

- [x] 3.3. Add npm scripts and documentation
  - Files: `tests/validation/package.json` (update), `tests/validation/README.md`
  - Add npm scripts: validate, validate:single, validate:report, validate:dry-run
  - Create README with usage instructions, examples, and troubleshooting
  - Document bug catalog YAML format
  - Add examples of running validation
  - Purpose: Make validation framework easy to use and understand
  - _Leverage: CLI implementation_
  - _Requirements: Non-functional (Usability)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Developer Experience Engineer | Task: Add npm scripts and comprehensive documentation following usability requirements, making framework easy to discover and use | Restrictions: Scripts should be intuitive (npm run validate), README should cover common use cases, provide troubleshooting section, document error messages, include examples | Success: npm run validate works, README is comprehensive and clear, examples are copy-pasteable, troubleshooting helps resolve common issues, bug catalog format is documented | Instructions: Mark in-progress, write thorough documentation, log with README sections created, mark complete._

### Phase 4: Testing and Validation

- [x] 4.1. Create unit tests for bug injector
  - Files: `tests/validation/__tests__/bug-injector.test.ts`
  - Test replace, delete, and comment strategies
  - Test error handling (file not found, pattern not found)
  - Test git operations (branch creation, cleanup)
  - Use fixture files for testing code mutations
  - Purpose: Ensure bug injection reliability
  - _Leverage: Jest or Mocha test framework, test fixtures_
  - _Requirements: Non-functional (Reliability)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Engineer specializing in unit testing | Task: Create comprehensive unit tests for BugInjector following reliability requirements, testing all strategies and error scenarios | Restrictions: Use real git operations with test repo, clean up after tests, test both success and failure cases, ensure tests are isolated and repeatable | Success: All bug injection strategies tested, error scenarios covered, git operations validated, tests run reliably and clean up properly | Instructions: Mark in-progress, write thorough tests, log with test coverage statistics, mark complete._

- [ ] 4.2. Create unit tests for test runner and orchestrator (DEFERRED)
  - Files: `tests/validation/__tests__/test-runner.test.ts`, `tests/validation/__tests__/orchestrator.test.ts`
  - _Note: Core framework is functional, unit tests deferred to focus on actual validation run_

- [ ] 4.3. Run end-to-end validation test (DEFERRED)
  - Files: `tests/validation/__tests__/e2e.test.ts`
  - _Note: Will validate via actual use in Phase 5 instead_

### Phase 5: Full Validation Run

- [-] 5.1. Run validation on all 50 tests
  - Files: `.validation-results/run-TIMESTAMP/` (generated)
  - Execute npm run validate to run full validation
  - Monitor progress and handle any failures
  - Capture complete validation report
  - Review STRONG vs WEAK test statistics
  - Purpose: Validate all 50 integration tests for effectiveness
  - _Leverage: Complete validation framework_
  - _Requirements: All_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Validation Lead | Task: Execute full validation run on all 50 tests covering all requirements, monitor execution, capture results, and analyze weak tests | Restrictions: Must run on clean repo state, monitor for git issues, watch for test failures, capture full output, verify cleanup after run | Success: Validation completes for all 50 tests, no git corruption, cleanup verified, comprehensive report generated, statistics show ≥90% STRONG tests (goal) | Instructions: Mark in-progress, monitor full run carefully, log with validation statistics and any issues found, mark complete._

- [ ] 5.2. Fix identified weak tests
  - Files: Tests identified as WEAK in validation report
  - Review validation report for WEAK tests
  - Analyze why each test didn't catch the bug
  - Implement fixes based on recommendations
  - Re-run validation to confirm fixes
  - Purpose: Improve weak tests to catch bugs effectively
  - _Leverage: Validation report recommendations_
  - _Requirements: REQ-6 (Test Improvement Recommendations)_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Improvement Engineer | Task: Fix all WEAK tests identified in validation report following recommendation REQ-6, adding missing assertions and improving test coverage | Restrictions: Follow recommendations from report, add specific assertions that would catch injected bugs, don't over-test, maintain test readability | Success: All previously WEAK tests now marked STRONG in re-validation, tests have proper assertions, validation report shows ≥95% STRONG tests | Instructions: Mark in-progress, fix tests systematically, log each test improvement with before/after comparison, mark complete._

- [ ] 5.3. Document validation results and best practices
  - Files: `.spec-workflow/specs/test-validation/validation-summary.md`
  - Create summary document with final statistics
  - Document lessons learned and best practices
  - Create guide for maintaining bug catalog as code evolves
  - Add examples of good vs weak tests
  - Purpose: Capture knowledge for future test writing
  - _Leverage: Validation reports, improved tests_
  - _Requirements: All_
  - _Prompt: Implement the task for spec test-validation, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Technical Documentation Specialist | Task: Document complete validation results and best practices covering all requirements, creating guide for maintaining test quality | Restrictions: Include concrete examples, provide actionable guidance, document common pitfalls, make it easy to follow for new developers | Success: Summary document is comprehensive, best practices are clear and actionable, bug catalog maintenance documented, examples illustrate points well | Instructions: Mark in-progress, document thoroughly, log with documentation sections created, mark complete._
