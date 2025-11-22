# Tasks Document: Integration Testing Framework

## Task Breakdown

### Phase 1: Project Setup and Configuration

- [ ] 1.1. Initialize test project with Node.js and TypeScript
  - Files: `tests/package.json`, `tests/tsconfig.json`, `tests/.gitignore`
  - Create package.json with Playwright and TypeScript dependencies
  - Configure TypeScript with strict mode and proper module resolution
  - Add .gitignore to exclude node_modules, reports, screenshots
  - Purpose: Establish test project foundation with proper tooling
  - _Requirements: REQ-5, REQ-6_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: DevOps Engineer specializing in Node.js project setup and tooling | Task: Initialize test project with package.json, tsconfig.json, and .gitignore following requirements REQ-5 and REQ-6, installing Playwright, TypeScript, and necessary dev dependencies | Restrictions: Use latest stable versions, enable TypeScript strict mode, follow Node.js best practices, do not include unnecessary dependencies | Success: package.json created with correct dependencies, tsconfig.json configured for test project, .gitignore prevents committing generated files, npm install runs successfully | Instructions: Before starting, mark this task as in-progress in tasks.md by changing `[ ]` to `[-]`. After completing implementation and testing, use the log-implementation tool with comprehensive artifacts (files created, dependencies installed). Finally, mark task as complete `[x]` in tasks.md._

- [ ] 1.2. Configure Playwright test runner
  - Files: `tests/playwright.config.ts`
  - Set up browser configuration (Chromium headless)
  - Configure base URL, timeouts, retries
  - Set up reporters (HTML, JSON)
  - Configure screenshot and video capture
  - Purpose: Establish test runner configuration for reliable test execution
  - _Requirements: REQ-1, REQ-5_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Automation Engineer with Playwright expertise | Task: Create comprehensive Playwright configuration following requirements REQ-1 and REQ-5, setting up browsers, timeouts, retries, and reporters with sensible defaults for both local and CI execution | Restrictions: Use headless Chromium, enable screenshots only on failure, configure appropriate timeouts (30s default), enable retry on failure (2 retries) | Success: playwright.config.ts created with proper configuration, tests can run in both local and CI modes, HTML and JSON reporters configured, screenshots/videos captured appropriately | Instructions: Mark in-progress, configure thoroughly, log implementation with configuration details, mark complete._

- [ ] 1.3. Set up ESLint and Prettier for test code
  - Files: `tests/.eslintrc.json`, `tests/.prettierrc.json`
  - Configure ESLint for TypeScript and Playwright
  - Set up Prettier for consistent formatting
  - Add npm scripts for linting and formatting
  - Purpose: Ensure test code quality and consistency
  - _Requirements: Non-functional (Maintainability)_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Code Quality Engineer specializing in linting and formatting tools | Task: Configure ESLint and Prettier for test project following maintainability requirements, adding appropriate rules for TypeScript and Playwright test code | Restrictions: Use recommended presets, do not conflict Prettier with ESLint, ensure rules enforce best practices without being overly strict | Success: .eslintrc.json and .prettierrc.json created with appropriate rules, npm run lint works correctly, npm run format fixes code style, test code follows consistent style | Instructions: Mark in-progress, configure linting rules, log implementation with tools configured, mark complete._

### Phase 2: Test Utilities and Helpers

- [ ] 2.1. Create server management utilities
  - Files: `tests/helpers/server.ts`
  - Implement startServer() function to launch Mango with cargo run
  - Implement waitForServerReady() to poll health endpoint
  - Implement stopServer() to gracefully shutdown
  - Add error handling for server startup failures
  - Purpose: Provide reliable server lifecycle management for tests
  - _Leverage: Node.js child_process module, Axum server health endpoint_
  - _Requirements: REQ-1, REQ-5_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Backend Test Infrastructure Engineer | Task: Create server management utilities following requirements REQ-1 and REQ-5, implementing functions to start Mango server via cargo run, wait for readiness, and stop gracefully | Restrictions: Use child_process.spawn, poll http://localhost:9000 for readiness, implement exponential backoff, capture server logs, handle startup failures gracefully | Success: startServer() launches Mango successfully, waitForServerReady() detects when server is ready (max 30s wait), stopServer() shuts down gracefully, error messages are clear when server fails to start | Instructions: Mark in-progress, implement with proper error handling, log with function signatures and usage examples, mark complete._

- [ ] 2.2. Create test utilities module
  - Files: `tests/helpers/test-utils.ts`
  - Implement captureEvidence(page, name) for screenshots
  - Implement waitForPageLoad(page) for stable page state
  - Implement clearBrowserState(page) to reset localStorage/cookies
  - Implement getConsoleErrors(page) to collect JS errors
  - Purpose: Provide reusable helper functions for common test operations
  - _Leverage: Playwright Page API_
  - _Requirements: REQ-4_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Utility Developer specializing in browser automation | Task: Create reusable test utilities following requirement REQ-4, implementing helpers for screenshots, page waits, browser state management, and error collection | Restrictions: Use Playwright best practices, handle errors gracefully, ensure utilities work across all test files, do not duplicate Playwright built-in functionality unnecessarily | Success: All utility functions work correctly, properly typed with TypeScript, well-documented with JSDoc comments, easy to use in test files | Instructions: Mark in-progress, implement comprehensive utilities, log with function signatures and examples, mark complete._

- [ ] 2.3. Create theme verification utilities
  - Files: `tests/helpers/theme-utils.ts`
  - Implement verifyTheme(page, expectedTheme) to check theme state
  - Implement toggleTheme(page) to click theme toggle button
  - Implement getThemeState(page) to get current theme from DOM and localStorage
  - Add helper to verify theme colors applied correctly
  - Purpose: Provide specialized utilities for theme testing
  - _Leverage: tests/helpers/test-utils.ts, Playwright Page API_
  - _Requirements: REQ-2_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Frontend Test Engineer specializing in UI testing | Task: Create theme verification utilities following requirement REQ-2, implementing functions to verify theme state, toggle theme, and validate color changes | Restrictions: Check both body.uk-light class and localStorage.getItem('theme'), verify navbar and card colors change, handle both light and dark modes, provide detailed error messages when verification fails | Success: verifyTheme() correctly validates theme state, toggleTheme() clicks toggle reliably, getThemeState() returns accurate current theme, color verification detects theme issues | Instructions: Mark in-progress, implement with visual verification, log with examples of theme checks, mark complete._

### Phase 3: Page Object Models

- [ ] 3.1. Create navigation component page object
  - Files: `tests/helpers/page-objects.ts`
  - Implement NavigationComponent class with selectors
  - Add methods: navigate(page), toggleTheme(), verifyActiveLink(page)
  - Add mobile menu methods: openMobileMenu(), closeMobileMenu()
  - Purpose: Encapsulate navigation interactions in reusable object
  - _Leverage: tests/helpers/theme-utils.ts_
  - _Requirements: REQ-2, REQ-4_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Page Object Pattern Expert | Task: Create NavigationComponent page object following requirements REQ-2 and REQ-4, encapsulating navigation selectors and interactions using page object pattern | Restrictions: Use data-testid or stable selectors, avoid XPath, make methods reusable, handle both desktop and mobile navigation, properly type all methods with TypeScript | Success: NavigationComponent class provides all navigation methods, selectors are reliable and stable, methods work for both desktop and mobile, properly typed and documented | Instructions: Mark in-progress, implement page object pattern correctly, log with class structure and methods, mark complete._

- [ ] 3.2. Create library page page object
  - Files: `tests/helpers/page-objects.ts` (continue)
  - Implement LibraryPage class for library interactions
  - Add methods: navigate(), search(query), selectSort(option), getTitleCards()
  - Add verification methods: verifyTitleExists(name), getTitleCount()
  - Purpose: Encapsulate library page interactions
  - _Leverage: tests/helpers/test-utils.ts_
  - _Requirements: REQ-1, REQ-4_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Page Object Developer | Task: Create LibraryPage page object following requirements REQ-1 and REQ-4, encapsulating library page selectors and search/sort interactions | Restrictions: Use stable selectors for search input and sort dropdown, handle async search results, provide clear verification methods, type all returns properly | Success: LibraryPage provides all library interactions, search and sort work reliably, verification methods accurate, well-typed and documented | Instructions: Mark in-progress, implement page object, log with methods and usage, mark complete._

- [ ] 3.3. Create reader page page object
  - Files: `tests/helpers/page-objects.ts` (continue)
  - Implement ReaderPage class for reader interactions
  - Add methods: verifyReaderLoaded(), navigatePage(direction), changeMode(mode)
  - Add settings methods: openSettings(), changeFit(option), toggleRTL()
  - Purpose: Encapsulate reader page interactions
  - _Leverage: tests/helpers/test-utils.ts_
  - _Requirements: REQ-3, REQ-4_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Reader Functionality Test Developer | Task: Create ReaderPage page object following requirements REQ-3 and REQ-4, encapsulating reader page selectors and reading mode/navigation interactions | Restrictions: Handle both paged and continuous modes, wait for images to load, verify no JavaScript errors, handle keyboard navigation, provide mode-switching methods | Success: ReaderPage provides all reader interactions, mode switching works, page navigation reliable, settings modal interactions correct, well-typed and documented | Instructions: Mark in-progress, implement reader page object, log with comprehensive methods, mark complete._

### Phase 4: Integration Test Suites

- [ ] 4.1. Create theme toggle test suite
  - Files: `tests/integration/theme.spec.ts`
  - Write tests for initial theme state detection
  - Test theme toggle on library page
  - Test theme persistence across page refresh
  - Test theme consistency across navigation
  - Capture before/after screenshots for visual regression
  - Purpose: Comprehensive theme system testing
  - _Leverage: tests/helpers/theme-utils.ts, tests/helpers/page-objects.ts_
  - _Requirements: REQ-2_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: UI Test Engineer specializing in theme testing | Task: Create comprehensive theme toggle test suite following requirement REQ-2, testing initial state, toggle functionality, persistence, and cross-page consistency using theme utilities and page objects | Restrictions: Test on multiple pages (library, book, home), verify both body.uk-light class and localStorage, capture screenshots for evidence, handle flaky tests with proper waits | Success: All theme tests pass reliably, covers light/dark toggle on all pages, verifies persistence after refresh, screenshots show visual changes, tests catch theme bugs like the one that was fixed | Instructions: Mark in-progress, write comprehensive tests, log with test coverage details, mark complete._

- [ ] 4.2. Create reader functionality test suite
  - Files: `tests/integration/reader.spec.ts`
  - Test reader page loads without JavaScript errors
  - Test paged mode navigation (arrow keys, click zones)
  - Test continuous mode with all images loading
  - Test reading mode switching (paged ↔ continuous)
  - Test settings modal (fit options, RTL, preload)
  - Purpose: Comprehensive reader functionality testing
  - _Leverage: tests/helpers/page-objects.ts (ReaderPage)_
  - _Requirements: REQ-3_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Reader Functionality QA Engineer | Task: Create comprehensive reader test suite following requirement REQ-3, testing page load, navigation, mode switching, and settings using ReaderPage page object | Restrictions: Navigate to real book entry, verify JavaScript loads without template variable errors, test keyboard shortcuts (arrow keys, space), verify images load in both modes, check settings persist to localStorage | Success: All reader tests pass, catches JavaScript errors, verifies navigation works, mode switching tested, settings changes verified, tests prevent bugs like reader.js template variable errors | Instructions: Mark in-progress, write thorough reader tests, log with comprehensive coverage, mark complete._

- [ ] 4.3. Create navigation test suite
  - Files: `tests/integration/navigation.spec.ts`
  - Test all main navigation links (Library, Tags, Admin)
  - Test active nav item highlighting
  - Test mobile hamburger menu on small viewport
  - Test page routing and URL changes
  - Verify each page loads correctly
  - Purpose: Comprehensive navigation testing
  - _Leverage: tests/helpers/page-objects.ts (NavigationComponent)_
  - _Requirements: REQ-1_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Navigation Test Engineer | Task: Create comprehensive navigation test suite following requirement REQ-1, testing all navigation links, active states, mobile menu, and page routing using NavigationComponent page object | Restrictions: Test both desktop and mobile viewports, verify URLs change correctly, check active class applied, ensure pages load fully, test hamburger menu opens/closes properly | Success: All navigation tests pass, covers desktop and mobile, active states verified, page routing correct, mobile menu functional, tests ensure navigation works across all pages | Instructions: Mark in-progress, write navigation tests, log with coverage across viewports, mark complete._

- [ ] 4.4. Create library search and sort test suite
  - Files: `tests/integration/library.spec.ts`
  - Test library page loads with all titles
  - Test search filters titles correctly
  - Test sort options (name, date, progress)
  - Test search clears when input cleared
  - Verify title cards render correctly
  - Purpose: Test library page functionality
  - _Leverage: tests/helpers/page-objects.ts (LibraryPage)_
  - _Requirements: REQ-1_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Library Functionality Test Engineer | Task: Create library test suite following requirement REQ-1, testing search, sort, and title display using LibraryPage page object | Restrictions: Test with real library data, verify search is case-insensitive, test all sort options work, ensure title cards have thumbnails and metadata, handle empty search results | Success: Library tests pass, search filters correctly, sort changes order, title cards display properly, tests verify library functionality works as expected | Instructions: Mark in-progress, implement library tests, log with search/sort test scenarios, mark complete._

### Phase 5: Test Fixtures and Setup

- [ ] 5.1. Create global test setup
  - Files: `tests/global-setup.ts`, `tests/global-teardown.ts`
  - Implement global setup to start Mango server once
  - Build CSS before tests run (./build-css.sh)
  - Implement global teardown to stop server
  - Clean up test artifacts (screenshots, reports)
  - Purpose: Optimize test execution with shared server instance
  - _Leverage: tests/helpers/server.ts_
  - _Requirements: REQ-5 (Test Organization and Execution)_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Infrastructure Engineer | Task: Create global test setup and teardown following requirement REQ-5, starting Mango server once for all tests and cleaning up afterward using server utilities | Restrictions: Run build-css.sh before starting server, wait for server ready before tests start, handle server startup failures gracefully, clean up all test artifacts in teardown | Success: global-setup.ts starts server successfully and builds CSS, global-teardown.ts stops server cleanly, test artifacts cleaned up, tests run faster with shared server | Instructions: Mark in-progress, implement setup/teardown, log with lifecycle management details, mark complete._

- [ ] 5.2. Create test fixtures for common scenarios
  - Files: `tests/helpers/fixtures.ts`
  - Create authenticated page fixture
  - Create page with specific theme fixture
  - Create page at specific URL fixture
  - Add cleanup handlers for each fixture
  - Purpose: Provide consistent test starting points
  - _Leverage: tests/helpers/test-utils.ts_
  - _Requirements: REQ-4_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Fixture Developer | Task: Create reusable test fixtures following requirement REQ-4, providing common starting states like authenticated page, themed page, and navigation to specific URLs | Restrictions: Use Playwright test.extend() for fixtures, ensure fixtures are isolated between tests, provide proper cleanup, type fixtures correctly with TypeScript | Success: Fixtures provide clean starting states, tests can easily start authenticated or at specific pages, fixtures clean up properly, well-typed and easy to use | Instructions: Mark in-progress, create fixtures, log with fixture types and usage patterns, mark complete._

### Phase 6: CI/CD Integration and Documentation

- [ ] 6.1. Create GitHub Actions workflow
  - Files: `.github/workflows/integration-tests.yml`
  - Set up workflow to run on pull requests
  - Install Rust, Node.js, and Playwright
  - Build CSS and run tests
  - Upload test reports and screenshots as artifacts
  - Purpose: Automate testing in CI pipeline
  - _Requirements: REQ-6_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CI/CD Engineer with GitHub Actions expertise | Task: Create GitHub Actions workflow following requirement REQ-6, automating integration tests on pull requests with proper setup and artifact upload | Restrictions: Install dependencies correctly (Rust, Node.js, Playwright with browsers), run build-css.sh before tests, upload reports and screenshots on both success and failure, block PRs if tests fail | Success: Workflow runs on PRs and pushes, installs all dependencies correctly, tests execute successfully, artifacts uploaded for debugging, PRs blocked when tests fail | Instructions: Mark in-progress, create workflow YAML, log with CI configuration details, mark complete._

- [ ] 6.2. Create README for test framework
  - Files: `tests/README.md`
  - Document prerequisites (Node.js, npm)
  - Explain how to install dependencies
  - Document how to run tests locally
  - Explain test organization and structure
  - Add troubleshooting section
  - Purpose: Help developers understand and use testing framework
  - _Requirements: Non-functional (Usability)_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Technical Documentation Writer | Task: Create comprehensive README for test framework following usability requirements, documenting setup, execution, organization, and troubleshooting | Restrictions: Keep documentation concise and practical, include code examples, cover common issues in troubleshooting, explain both local and CI execution | Success: README is complete and clear, developers can set up and run tests following the docs, troubleshooting helps solve common issues, examples are accurate | Instructions: Mark in-progress, write thorough documentation, log with documentation structure, mark complete._

- [ ] 6.3. Add npm scripts for test execution
  - Files: `tests/package.json` (modify)
  - Add script for running all tests: `npm test`
  - Add script for specific suite: `npm run test:theme`
  - Add script for headed mode (debugging): `npm run test:headed`
  - Add script for generating report: `npm run test:report`
  - Purpose: Provide convenient test execution commands
  - _Requirements: REQ-5_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Build Tool Configuration Expert | Task: Add npm scripts to package.json following requirement REQ-5, providing convenient commands for running tests in various modes | Restrictions: Use Playwright CLI commands, support headless and headed modes, allow running specific suites, provide report viewing script, keep scripts simple and maintainable | Success: npm test runs all tests in headless mode, npm run test:headed shows browser, suite-specific scripts work (test:theme, test:reader), npm run test:report opens HTML report | Instructions: Mark in-progress, add comprehensive scripts, log with script commands and usage, mark complete._

### Phase 7: Testing and Validation

- [ ] 7.1. Run full test suite and fix issues
  - Task: Execute all integration tests
  - Fix any failing tests or flaky tests
  - Verify all tests pass consistently (3+ runs)
  - Capture evidence of successful test run
  - Purpose: Ensure test framework is robust and reliable
  - _Leverage: All previous tasks_
  - _Requirements: All requirements_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Engineer specializing in test reliability | Task: Execute full test suite covering all requirements, identify and fix any failing or flaky tests, ensure consistent passing results | Restrictions: Run tests multiple times to catch flakiness, fix root causes not symptoms, ensure all tests pass in both local and CI environments, verify test evidence captured correctly | Success: All tests pass consistently across 3+ runs, no flaky tests, HTML report shows 100% pass rate, screenshots captured on any failures for debugging | Instructions: Mark in-progress, run and fix tests, log with test execution results and fixes made, mark complete._

- [ ] 7.2. Validate test framework prevents regressions
  - Task: Manually introduce theme toggle bug and verify tests catch it
  - Introduce reader JavaScript error and verify tests catch it
  - Verify tests provide useful debugging information
  - Purpose: Confirm test framework achieves its goal
  - _Leverage: All test suites_
  - _Requirements: All requirements_
  - _Prompt: Implement the task for spec integration-testing, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Test Validation Engineer | Task: Validate test framework prevents regressions covering all requirements by intentionally introducing bugs similar to recent issues and verifying tests detect them | Restrictions: Test both theme toggle bug (reader CSS overriding theme) and reader JavaScript bug (template variables), verify test failure messages are clear, confirm screenshots/evidence help debugging, restore code after validation | Success: Tests catch intentionally introduced theme bug, tests catch reader JavaScript errors, test failures provide clear error messages and evidence, framework proves it would prevent these bugs in future | Instructions: Mark in-progress, validate thoroughly, log with validation scenarios and results, mark complete._

## Summary

**Total Tasks:** 23 tasks across 7 phases
**Estimated Time:** 3-4 days of focused work for experienced test engineer
**Order:** Must be completed sequentially within phases, some tasks within phases can be parallelized

**Success Metrics:**
- All 23 tasks completed and marked [x]
- Integration tests run successfully in both local and CI environments
- Test suite catches theme and reader bugs when introduced
- HTML reports show comprehensive coverage
- Documentation enables other developers to add tests easily
- CI pipeline blocks PRs when integration tests fail

**Key Deliverables:**
1. Playwright-based integration testing framework
2. Reusable page objects and test utilities
3. Comprehensive test suites for theme, reader, navigation, library
4. CI/CD integration with GitHub Actions
5. Complete documentation in tests/README.md
