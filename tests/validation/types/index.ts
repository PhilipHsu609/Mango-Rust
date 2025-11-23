/**
 * Type definitions for the test validation framework
 * Defines interfaces for bug injection, test execution, and validation results
 */

/**
 * Bug injection strategy
 * - replace: Find and replace exact string
 * - delete: Remove matching lines
 * - comment: Comment out matching lines
 */
export type BugStrategy = 'replace' | 'delete' | 'comment';

/**
 * Validation status indicating test strength
 * - STRONG: Test failed when bug was injected (good - test caught the bug)
 * - WEAK: Test passed despite bug injection (bad - test missed the bug)
 * - INJECTION_FAILED: Bug couldn't be injected (file/pattern not found)
 * - TIMEOUT: Test execution exceeded time limit
 * - CRASH: Test crashed or threw unexpected error
 */
export type ValidationStatus = 'STRONG' | 'WEAK' | 'INJECTION_FAILED' | 'TIMEOUT' | 'CRASH';

/**
 * Bug definition from catalog
 */
export interface BugDefinition {
    /** Unique bug identifier */
    id: string;

    /** Human-readable description of what this bug breaks */
    description: string;

    /** File path relative to project root */
    targetFile: string;

    /** Bug injection strategy */
    strategy: BugStrategy;

    /** Exact string to find in the file */
    find: string;

    /** Replacement string (required for 'replace' strategy) */
    replace?: string;

    /** Hint about which assertion should fail */
    expectedFailure?: string;
}

/**
 * Test entry from bug catalog
 */
export interface TestValidationEntry {
    /** Unique test identifier (file:line:column) */
    testId: string;

    /** Human-readable test name */
    testName: string;

    /** Test file path */
    testFile: string;

    /** What functionality the test validates */
    functionality: string;

    /** List of bugs to inject for this test */
    bugs: BugDefinition[];
}

/**
 * Bug catalog structure (parsed from YAML)
 */
export interface BugCatalog {
    /** Catalog version */
    version: string;

    /** Generation timestamp */
    generatedAt: string;

    /** List of test validation entries */
    tests: TestValidationEntry[];
}

/**
 * Git branch information
 */
export interface BranchInfo {
    /** Branch name (e.g., "test-validation/theme-toggle/bug-1") */
    branchName: string;

    /** Original branch we came from */
    originalBranch: string;

    /** Bug ID that was injected */
    bugId: string;

    /** Timestamp when branch was created */
    timestamp: number;
}

/**
 * Test specification for execution
 */
export interface TestSpec {
    /** Test file name */
    file: string;

    /** Full test name */
    testName: string;

    /** Full test ID (file:line:column) */
    fullId: string;
}

/**
 * Test execution result
 */
export interface TestResult {
    /** Whether test passed */
    passed: boolean;

    /** Test duration in milliseconds */
    duration: number;

    /** Process exit code */
    exitCode: number;

    /** Standard output */
    stdout: string;

    /** Standard error */
    stderr: string;

    /** Error message if test crashed */
    error?: string;
}

/**
 * Validation result for a single bug injection
 */
export interface ValidationResult {
    /** Test ID */
    testId: string;

    /** Test name */
    testName: string;

    /** Bug ID that was injected */
    bugId: string;

    /** Bug description */
    bugDescription: string;

    /** Whether test passed (should be false for STRONG test) */
    testPassed: boolean;

    /** Validation status */
    validation: ValidationStatus;

    /** Duration in milliseconds */
    duration: number;

    /** Error message if validation failed */
    error?: string;

    /** Test output */
    output: string;

    /** Branch information */
    branchInfo?: BranchInfo;
}

/**
 * Complete validation run results
 */
export interface ValidationRun {
    /** Run timestamp (ISO string) */
    timestamp: string;

    /** Total number of tests validated */
    totalTests: number;

    /** Number of STRONG tests (caught bugs) */
    strongTests: number;

    /** Number of WEAK tests (missed bugs) */
    weakTests: number;

    /** Number of failed injections */
    injectionFailed: number;

    /** Number of timeouts */
    timeouts: number;

    /** Number of crashes */
    crashes: number;

    /** Individual validation results */
    results: ValidationResult[];

    /** Total duration in milliseconds */
    duration: number;
}

/**
 * Weak test report with recommendations
 */
export interface WeakTestReport {
    /** Test ID */
    testId: string;

    /** Test name */
    testName: string;

    /** Bug that was missed */
    bugDescription: string;

    /** Explanation of why test is weak */
    whyWeak: string;

    /** Recommendation for improvement */
    recommendation: string;

    /** Optional code snippet showing the fix */
    codeSnippet?: string;
}

/**
 * Strong test report (for completeness)
 */
export interface StrongTestReport {
    /** Test ID */
    testId: string;

    /** Test name */
    testName: string;

    /** Bug that was caught */
    bugDescription: string;

    /** Brief confirmation message */
    message: string;
}

/**
 * Validation report metadata
 */
export interface ReportMetadata {
    /** Report generation timestamp */
    generatedAt: string;

    /** Validation run timestamp */
    runTimestamp: string;

    /** Total validation duration */
    duration: number;

    /** Project path */
    projectPath: string;
}

/**
 * Validation summary statistics
 */
export interface ValidationSummary {
    /** Total tests validated */
    totalTests: number;

    /** Strong tests count */
    strongTests: number;

    /** Weak tests count */
    weakTests: number;

    /** Injection failures */
    injectionFailed: number;

    /** Timeouts */
    timeouts: number;

    /** Crashes */
    crashes: number;

    /** Coverage percentage (strong / total) */
    coverage: number;

    /** Total duration */
    duration: number;
}

/**
 * Complete validation report
 */
export interface ValidationReport {
    /** Summary statistics */
    summary: ValidationSummary;

    /** Weak test reports */
    weakTests: WeakTestReport[];

    /** Strong test reports */
    strongTests: StrongTestReport[];

    /** Report metadata */
    metadata: ReportMetadata;
}
