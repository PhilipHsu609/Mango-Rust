/**
 * Test Runner - Executes Playwright tests and captures results
 * Runs individual tests via npx playwright with proper filtering and timeout handling
 */

import { spawn } from 'child_process';
import { TestSpec, TestResult } from '../types/index.js';

/**
 * Test Runner class for executing Playwright tests
 */
export class TestRunner {
    private testsPath: string;
    private timeout: number;

    /**
     * @param testsPath Path to tests directory (default: 'tests')
     * @param timeout Test timeout in milliseconds (default: 60000)
     */
    constructor(testsPath: string = 'tests', timeout: number = 60000) {
        this.testsPath = testsPath;
        this.timeout = timeout;
    }

    /**
     * Run a single test using Playwright
     * @param spec Test specification
     * @returns Test result with pass/fail status and output
     */
    async runTest(spec: TestSpec): Promise<TestResult> {
        const startTime = Date.now();

        try {
            // Execute test with grep filter for specific test
            const result = await this.executePlaywrightTest(spec);

            const duration = Date.now() - startTime;

            return {
                passed: result.exitCode === 0,
                duration,
                exitCode: result.exitCode,
                stdout: result.stdout,
                stderr: result.stderr,
                error: result.error,
            };
        } catch (error) {
            const duration = Date.now() - startTime;

            return {
                passed: false,
                duration,
                exitCode: -1,
                stdout: '',
                stderr: '',
                error: `Test execution failed: ${error}`,
            };
        }
    }

    /**
     * Execute Playwright test via npx
     */
    private executePlaywrightTest(spec: TestSpec): Promise<{
        exitCode: number;
        stdout: string;
        stderr: string;
        error?: string;
    }> {
        return new Promise((resolve) => {
            let stdout = '';
            let stderr = '';
            let timedOut = false;

            // Use --grep to run only the specific test
            // Escape special regex characters in test name
            const escapedTestName = this.escapeRegex(spec.testName);

            const args = [
                'playwright',
                'test',
                '--grep',
                escapedTestName,
                '--reporter=list',
            ];

            const child = spawn('npx', args, {
                cwd: this.testsPath,
                env: { ...process.env },
                stdio: ['ignore', 'pipe', 'pipe'],
            });

            // Set timeout
            const timeoutHandle = setTimeout(() => {
                timedOut = true;
                child.kill('SIGTERM');

                // Force kill after 5 seconds if still running
                setTimeout(() => {
                    if (!child.killed) {
                        child.kill('SIGKILL');
                    }
                }, 5000);
            }, this.timeout);

            // Collect stdout
            child.stdout?.on('data', (data) => {
                stdout += data.toString();
            });

            // Collect stderr
            child.stderr?.on('data', (data) => {
                stderr += data.toString();
            });

            // Handle completion
            child.on('close', (code) => {
                clearTimeout(timeoutHandle);

                if (timedOut) {
                    resolve({
                        exitCode: 124, // timeout exit code
                        stdout,
                        stderr,
                        error: `Test exceeded timeout of ${this.timeout}ms`,
                    });
                } else {
                    resolve({
                        exitCode: code ?? -1,
                        stdout,
                        stderr,
                    });
                }
            });

            // Handle errors
            child.on('error', (error) => {
                clearTimeout(timeoutHandle);
                resolve({
                    exitCode: -1,
                    stdout,
                    stderr,
                    error: `Failed to spawn test process: ${error.message}`,
                });
            });
        });
    }

    /**
     * Escape special regex characters for --grep
     */
    private escapeRegex(str: string): string {
        return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    }

    /**
     * Parse test output to determine if test passed
     * This is a backup check in addition to exit code
     */
    parseTestOutput(output: string): { passed: boolean; summary: string } {
        // Check for explicit pass indicators
        const passIndicators = [
            /✓.*passed/i,
            /\d+\s+passed/i,
            /All tests passed/i,
        ];

        const failIndicators = [
            /✗.*failed/i,
            /\d+\s+failed/i,
            /Error:/i,
            /FAILED/i,
        ];

        let passed = true;
        let summary = 'Test completed';

        // Check for failure indicators
        for (const indicator of failIndicators) {
            if (indicator.test(output)) {
                passed = false;
                summary = 'Test failed';
                break;
            }
        }

        // If no failures found, look for pass indicators
        if (passed) {
            for (const indicator of passIndicators) {
                if (indicator.test(output)) {
                    summary = 'Test passed';
                    break;
                }
            }
        }

        return { passed, summary };
    }

    /**
     * Extract test name from test ID
     * @param testId Test ID in format "file:line:column"
     */
    static extractTestName(testId: string): string {
        // This is a simple extraction - in practice, you might need to read the file
        // For now, just return the testId as-is
        return testId;
    }

    /**
     * Check if Playwright is available
     */
    async checkPlaywrightAvailable(): Promise<boolean> {
        return new Promise((resolve) => {
            const child = spawn('npx', ['playwright', '--version'], {
                cwd: this.testsPath,
                stdio: 'ignore',
            });

            child.on('close', (code) => {
                resolve(code === 0);
            });

            child.on('error', () => {
                resolve(false);
            });
        });
    }

    /**
     * Get test execution statistics
     */
    getTestStats(result: TestResult): {
        passed: boolean;
        duration: number;
        durationFormatted: string;
        hasError: boolean;
        hasOutput: boolean;
    } {
        const durationSec = (result.duration / 1000).toFixed(2);

        return {
            passed: result.passed,
            duration: result.duration,
            durationFormatted: `${durationSec}s`,
            hasError: !!result.error,
            hasOutput: result.stdout.length > 0 || result.stderr.length > 0,
        };
    }
}
