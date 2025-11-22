# Requirements Document: Integration Testing Framework

## Introduction

Mango-Rust requires a comprehensive integration testing framework to ensure that frontend-backend interactions, theme system, and reader functionality work correctly across all pages. This framework will enable automated, reusable test flows that catch bugs like the recent theme toggle and reader JavaScript issues before they reach production.

The testing framework will leverage Playwright for browser automation and provide reusable test patterns that can be executed during development, in CI/CD pipelines, and before releases.

## Alignment with Product Vision

This feature supports the Mango-Rust project's commitment to code quality and maintainability by:
- **Preventing regressions**: Automated tests catch bugs introduced by refactoring
- **Accelerating development**: Developers can quickly verify changes work correctly
- **Improving reliability**: Users experience fewer bugs and broken features
- **Enabling confident refactoring**: Comprehensive test coverage allows safe code improvements

## Requirements

### REQ-1: Browser-Based Integration Testing

**User Story:** As a developer, I want automated browser-based integration tests, so that I can verify frontend-backend interactions work correctly.

#### Acceptance Criteria

1. WHEN integration tests run THEN the system SHALL launch a real browser and interact with the running Mango server
2. WHEN tests execute THEN the system SHALL navigate between pages (library, book, home, reader, tags, admin)
3. WHEN tests complete THEN the system SHALL report pass/fail status with detailed error messages
4. IF a test fails THEN the system SHALL capture screenshots and console logs for debugging

### REQ-2: Theme System Testing

**User Story:** As a developer, I want automated theme toggle tests, so that I can ensure light/dark mode switching works correctly on all pages.

#### Acceptance Criteria

1. WHEN theme tests run THEN the system SHALL verify initial theme state (light or dark)
2. WHEN theme toggle is clicked THEN the system SHALL verify body class changes to `uk-light` for dark mode
3. WHEN theme toggle is clicked THEN the system SHALL verify all page elements switch colors correctly (navbar, cards, text, backgrounds)
4. WHEN page refreshes THEN the system SHALL verify theme preference persists via localStorage
5. WHEN navigating between pages THEN the system SHALL verify theme remains consistent
6. IF theme toggle fails THEN the system SHALL capture before/after screenshots showing the visual difference

### REQ-3: Reader Functionality Testing

**User Story:** As a developer, I want automated reader page tests, so that I can verify comic reading functionality works correctly.

#### Acceptance Criteria

1. WHEN reader tests run THEN the system SHALL verify reader page loads without JavaScript errors
2. WHEN reader page loads THEN the system SHALL verify images load in correct reading mode (paged or continuous)
3. WHEN keyboard navigation is triggered THEN the system SHALL verify page changes work correctly (arrow keys, space)
4. WHEN reading mode changes THEN the system SHALL verify UI switches between paged and continuous modes
5. WHEN reader settings are changed THEN the system SHALL verify preferences save to localStorage
6. IF reader JavaScript has template variable errors THEN the system SHALL report the specific error location

### REQ-4: Reusable Test Utilities

**User Story:** As a developer, I want reusable test helper functions, so that I can write integration tests efficiently without code duplication.

#### Acceptance Criteria

1. WHEN writing new tests THEN developers SHALL have access to common utilities (login, navigate, wait, screenshot)
2. WHEN testing theme THEN developers SHALL use `verifyTheme(expectedTheme)` helper function
3. WHEN testing pages THEN developers SHALL use `navigateAndWait(page, url)` helper function
4. WHEN capturing evidence THEN developers SHALL use `captureTestEvidence(name)` helper function
5. IF test utilities are updated THEN existing tests SHALL continue to work without modification (backward compatibility)

### REQ-5: Test Organization and Execution

**User Story:** As a developer, I want organized test suites, so that I can run specific test categories independently.

#### Acceptance Criteria

1. WHEN running tests THEN developers SHALL be able to run all tests with single command
2. WHEN running tests THEN developers SHALL be able to run specific suites (theme, reader, navigation)
3. WHEN tests execute THEN the system SHALL display progress and results in clear format
4. WHEN tests complete THEN the system SHALL generate HTML test report with screenshots
5. IF any test fails THEN the system SHALL exit with non-zero status for CI/CD integration

### REQ-6: CI/CD Integration

**User Story:** As a DevOps engineer, I want tests to run in CI/CD pipeline, so that pull requests are automatically validated.

#### Acceptance Criteria

1. WHEN PR is created THEN CI SHALL automatically run integration test suite
2. WHEN tests run in CI THEN the system SHALL use headless browser mode
3. WHEN tests complete in CI THEN the system SHALL upload test artifacts (screenshots, reports) to CI platform
4. IF tests fail in CI THEN PR SHALL be blocked from merging
5. WHEN tests pass in CI THEN PR SHALL show green checkmark

## Non-Functional Requirements

### Code Architecture and Modularity

- **Single Responsibility Principle**: Each test file tests one feature area (theme, reader, navigation)
- **Modular Design**: Test utilities are isolated in separate helper modules
- **Dependency Management**: Tests depend only on Playwright and test utilities, not implementation details
- **Clear Interfaces**: Test helper functions have clear contracts and documentation

### Performance

- **Test Execution Speed**: Full test suite SHOULD complete in under 2 minutes
- **Parallel Execution**: Tests SHOULD run in parallel where possible to minimize execution time
- **Resource Efficiency**: Tests SHALL clean up browser resources after execution

### Reliability

- **Flake Reduction**: Tests SHALL use proper waits and retries to minimize false failures
- **Deterministic Results**: Tests SHALL produce consistent results across multiple runs
- **Error Recovery**: Tests SHALL handle network errors and timeouts gracefully

### Maintainability

- **Clear Test Names**: Test descriptions SHALL clearly explain what is being tested
- **Minimal Duplication**: Common test patterns SHALL be extracted to reusable utilities
- **Easy Debugging**: Test failures SHALL provide enough context to diagnose issues quickly
- **Documentation**: Test framework SHALL include README with setup and usage instructions

### Usability

- **Simple Setup**: Developers SHALL install test dependencies with single npm/cargo command
- **Quick Feedback**: Tests SHALL provide immediate feedback during development with watch mode
- **Clear Output**: Test results SHALL be easy to read and understand
