/**
 * Validation Orchestrator - Coordinates the complete validation workflow
 * Manages bug injection, test execution, result validation, and cleanup
 */

import * as fs from 'fs/promises';
import * as path from 'path';
import * as yaml from 'yaml';
import ora, { Ora } from 'ora';
import chalk from 'chalk';
import { SimpleGit } from 'simple-git';
import {
    BugCatalog,
    TestValidationEntry,
    BugDefinition,
    ValidationResult,
    ValidationRun,
    ValidationStatus,
    TestSpec,
} from '../types/index.js';
import { createGit, verifyCleanRepository } from '../utils/git-utils.js';
import { BugInjector } from './bug-injector.js';
import { TestRunner } from './test-runner.js';

/**
 * Validation Orchestrator Options
 */
export interface OrchestratorOptions {
    /** Path to project root */
    projectPath: string;

    /** Path to bug catalog YAML */
    catalogPath: string;

    /** Path to tests directory */
    testsPath?: string;

    /** Test timeout in milliseconds */
    timeout?: number;

    /** Show verbose output */
    verbose?: boolean;
}

/**
 * Validation Orchestrator - Main workflow coordinator
 */
export class ValidationOrchestrator {
    private projectPath: string;
    private catalogPath: string;
    private testsPath: string;
    private timeout: number;
    private verbose: boolean;
    private git: SimpleGit;
    private bugInjector: BugInjector;
    private testRunner: TestRunner;

    constructor(options: OrchestratorOptions) {
        this.projectPath = options.projectPath;
        this.catalogPath = options.catalogPath;
        this.testsPath = options.testsPath || 'tests';
        this.timeout = options.timeout || 60000;
        this.verbose = options.verbose || false;

        // Initialize components
        this.git = createGit(this.projectPath);
        this.bugInjector = new BugInjector(this.git, this.projectPath);
        this.testRunner = new TestRunner(this.testsPath, this.timeout);
    }

    /**
     * Validate all tests in the bug catalog
     * @returns Complete validation run results
     */
    async validateAll(): Promise<ValidationRun> {
        const startTime = Date.now();

        // Load bug catalog
        const catalog = await this.loadCatalog();

        // Verify repository is clean
        await verifyCleanRepository(this.git);

        // Check Playwright is available
        const playwrightAvailable = await this.testRunner.checkPlaywrightAvailable();
        if (!playwrightAvailable) {
            throw new Error('Playwright not available. Run: npm install --prefix tests');
        }

        console.log(chalk.bold('\n🧪 Test Validation Framework\n'));
        console.log(`Catalog: ${catalog.tests.length} tests`);
        console.log(`Total bugs: ${this.countTotalBugs(catalog)}`);
        console.log(`Project: ${this.projectPath}\n`);

        const results: ValidationResult[] = [];

        // Validate each test
        for (let i = 0; i < catalog.tests.length; i++) {
            const test = catalog.tests[i];
            const testResults = await this.validateTest(test, i + 1, catalog.tests.length);
            results.push(...testResults);
        }

        const duration = Date.now() - startTime;

        // Calculate statistics
        const run = this.buildValidationRun(results, duration);

        // Display summary
        this.displaySummary(run);

        return run;
    }

    /**
     * Validate a single test with all its bugs
     * @param test Test validation entry
     * @param index Current test index (for progress)
     * @param total Total tests count (for progress)
     * @returns Array of validation results
     */
    async validateTest(
        test: TestValidationEntry,
        index?: number,
        total?: number
    ): Promise<ValidationResult[]> {
        const prefix = index && total ? `[${index}/${total}] ` : '';
        const testLabel = `${prefix}${test.testName}`;

        console.log(chalk.cyan(`\n${testLabel}`));
        console.log(chalk.gray(`  ${test.functionality}`));

        const results: ValidationResult[] = [];

        // Validate each bug for this test
        for (let i = 0; i < test.bugs.length; i++) {
            const bug = test.bugs[i];
            const bugLabel = `  Bug ${i + 1}/${test.bugs.length}: ${bug.description}`;

            const spinner = ora({
                text: bugLabel,
                color: 'yellow',
            }).start();

            try {
                const result = await this.validateSingleBug(test, bug);
                results.push(result);

                // Update spinner based on result
                if (result.validation === 'STRONG') {
                    spinner.succeed(chalk.green(`${bugLabel} → STRONG ✓`));
                } else if (result.validation === 'WEAK') {
                    spinner.fail(chalk.red(`${bugLabel} → WEAK (test passed despite bug)`));
                } else if (result.validation === 'INJECTION_FAILED') {
                    spinner.warn(chalk.yellow(`${bugLabel} → INJECTION FAILED`));
                } else if (result.validation === 'TIMEOUT') {
                    spinner.warn(chalk.yellow(`${bugLabel} → TIMEOUT`));
                } else if (result.validation === 'CRASH') {
                    spinner.fail(chalk.red(`${bugLabel} → CRASH`));
                }
            } catch (error) {
                spinner.fail(chalk.red(`${bugLabel} → ERROR`));

                // Create error result
                results.push({
                    testId: test.testId,
                    testName: test.testName,
                    bugId: bug.id,
                    bugDescription: bug.description,
                    testPassed: false,
                    validation: 'CRASH',
                    duration: 0,
                    error: String(error),
                    output: '',
                });

                if (this.verbose) {
                    console.log(chalk.gray(`    Error: ${error}`));
                }
            }
        }

        return results;
    }

    /**
     * Validate a single bug injection
     * @param test Test validation entry
     * @param bug Bug definition
     * @returns Validation result
     */
    private async validateSingleBug(
        test: TestValidationEntry,
        bug: BugDefinition
    ): Promise<ValidationResult> {
        const startTime = Date.now();
        let branchInfo;

        try {
            // Step 1: Inject bug (creates branch)
            try {
                branchInfo = await this.bugInjector.injectBug(bug, test.testId);
            } catch (error) {
                // Bug injection failed
                return {
                    testId: test.testId,
                    testName: test.testName,
                    bugId: bug.id,
                    bugDescription: bug.description,
                    testPassed: false,
                    validation: 'INJECTION_FAILED',
                    duration: Date.now() - startTime,
                    error: String(error),
                    output: '',
                };
            }

            // Step 2: Run test
            const testSpec: TestSpec = {
                file: test.testFile,
                testName: test.testName,
                fullId: test.testId,
            };

            const testResult = await this.testRunner.runTest(testSpec);
            const duration = Date.now() - startTime;

            // Step 3: Determine validation status
            let validation: ValidationStatus;

            if (testResult.error?.includes('timeout')) {
                validation = 'TIMEOUT';
            } else if (testResult.exitCode === -1 || testResult.error) {
                validation = 'CRASH';
            } else if (testResult.passed) {
                // Test passed despite bug injection → WEAK
                validation = 'WEAK';
            } else {
                // Test failed when bug was injected → STRONG
                validation = 'STRONG';
            }

            // Step 4: Cleanup branch
            await this.bugInjector.revertBug(branchInfo);

            return {
                testId: test.testId,
                testName: test.testName,
                bugId: bug.id,
                bugDescription: bug.description,
                testPassed: testResult.passed,
                validation,
                duration,
                error: testResult.error,
                output: testResult.stdout + testResult.stderr,
                branchInfo,
            };
        } catch (error) {
            // Ensure cleanup even on unexpected errors
            if (branchInfo) {
                try {
                    await this.bugInjector.revertBug(branchInfo);
                } catch (cleanupError) {
                    console.error(chalk.red(`Failed to cleanup branch: ${cleanupError}`));
                }
            }

            throw error;
        }
    }

    /**
     * Load bug catalog from YAML file
     */
    private async loadCatalog(): Promise<BugCatalog> {
        try {
            const content = await fs.readFile(this.catalogPath, 'utf-8');
            const catalog = yaml.parse(content) as BugCatalog;

            // Validate catalog structure
            if (!catalog.tests || !Array.isArray(catalog.tests)) {
                throw new Error('Invalid catalog: missing or invalid "tests" array');
            }

            return catalog;
        } catch (error) {
            throw new Error(`Failed to load bug catalog: ${error}`);
        }
    }

    /**
     * Count total bugs across all tests
     */
    private countTotalBugs(catalog: BugCatalog): number {
        return catalog.tests.reduce((sum, test) => sum + test.bugs.length, 0);
    }

    /**
     * Build ValidationRun from results
     */
    private buildValidationRun(results: ValidationResult[], duration: number): ValidationRun {
        const strongTests = results.filter(r => r.validation === 'STRONG').length;
        const weakTests = results.filter(r => r.validation === 'WEAK').length;
        const injectionFailed = results.filter(r => r.validation === 'INJECTION_FAILED').length;
        const timeouts = results.filter(r => r.validation === 'TIMEOUT').length;
        const crashes = results.filter(r => r.validation === 'CRASH').length;

        return {
            timestamp: new Date().toISOString(),
            totalTests: results.length,
            strongTests,
            weakTests,
            injectionFailed,
            timeouts,
            crashes,
            results,
            duration,
        };
    }

    /**
     * Display validation summary
     */
    private displaySummary(run: ValidationRun): void {
        console.log(chalk.bold('\n📊 Validation Summary\n'));

        const coverage = run.totalTests > 0
            ? ((run.strongTests / run.totalTests) * 100).toFixed(1)
            : '0.0';

        console.log(`Total validations: ${run.totalTests}`);
        console.log(chalk.green(`✓ STRONG tests:    ${run.strongTests} (${((run.strongTests / run.totalTests) * 100).toFixed(1)}%)`));
        console.log(chalk.red(`✗ WEAK tests:      ${run.weakTests} (${((run.weakTests / run.totalTests) * 100).toFixed(1)}%)`));

        if (run.injectionFailed > 0) {
            console.log(chalk.yellow(`⚠ Injection failed: ${run.injectionFailed}`));
        }
        if (run.timeouts > 0) {
            console.log(chalk.yellow(`⏱ Timeouts:         ${run.timeouts}`));
        }
        if (run.crashes > 0) {
            console.log(chalk.red(`💥 Crashes:         ${run.crashes}`));
        }

        console.log(`\nTest effectiveness: ${coverage}%`);
        console.log(`Total duration: ${(run.duration / 1000).toFixed(1)}s\n`);

        // Show goal progress
        const goalCoverage = 90;
        if (parseFloat(coverage) >= goalCoverage) {
            console.log(chalk.green(`🎉 Goal achieved! Coverage ≥ ${goalCoverage}%\n`));
        } else {
            console.log(chalk.yellow(`🎯 Goal: ${goalCoverage}% coverage (${(goalCoverage - parseFloat(coverage)).toFixed(1)}% to go)\n`));
        }
    }

    /**
     * Get validation statistics for a test
     */
    getTestStats(testId: string, results: ValidationResult[]): {
        testId: string;
        totalBugs: number;
        strong: number;
        weak: number;
        failed: number;
        coverage: number;
    } {
        const testResults = results.filter(r => r.testId === testId);
        const strong = testResults.filter(r => r.validation === 'STRONG').length;
        const weak = testResults.filter(r => r.validation === 'WEAK').length;
        const failed = testResults.filter(r =>
            r.validation === 'INJECTION_FAILED' ||
            r.validation === 'TIMEOUT' ||
            r.validation === 'CRASH'
        ).length;

        return {
            testId,
            totalBugs: testResults.length,
            strong,
            weak,
            failed,
            coverage: testResults.length > 0 ? (strong / testResults.length) * 100 : 0,
        };
    }

    /**
     * Filter results by validation status
     */
    filterResults(results: ValidationResult[], status: ValidationStatus): ValidationResult[] {
        return results.filter(r => r.validation === status);
    }

    /**
     * Group results by test
     */
    groupByTest(results: ValidationResult[]): Map<string, ValidationResult[]> {
        const grouped = new Map<string, ValidationResult[]>();

        for (const result of results) {
            const existing = grouped.get(result.testId) || [];
            existing.push(result);
            grouped.set(result.testId, existing);
        }

        return grouped;
    }

    /**
     * Save validation run to JSON file
     */
    async saveResults(run: ValidationRun, outputPath: string): Promise<void> {
        await fs.mkdir(path.dirname(outputPath), { recursive: true });
        await fs.writeFile(outputPath, JSON.stringify(run, null, 2), 'utf-8');
    }

    /**
     * Load validation run from JSON file
     */
    async loadResults(inputPath: string): Promise<ValidationRun> {
        const content = await fs.readFile(inputPath, 'utf-8');
        return JSON.parse(content) as ValidationRun;
    }
}
