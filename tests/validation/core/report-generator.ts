/**
 * Report Generator - Creates comprehensive validation reports
 * Generates both markdown (human-readable) and JSON (machine-readable) reports
 */

import * as fs from 'fs/promises';
import * as path from 'path';
import {
    ValidationRun,
    ValidationResult,
    ValidationReport,
    WeakTestReport,
    StrongTestReport,
    ValidationSummary,
    ReportMetadata,
} from '../types/index.js';

/**
 * Report Generator Options
 */
export interface ReportGeneratorOptions {
    /** Path to project root */
    projectPath: string;

    /** Include detailed test output in reports */
    includeOutput?: boolean;

    /** Include code snippets in recommendations */
    includeCodeSnippets?: boolean;
}

/**
 * Report Generator class for creating validation reports
 */
export class ReportGenerator {
    private projectPath: string;
    private includeOutput: boolean;
    private includeCodeSnippets: boolean;

    constructor(options: ReportGeneratorOptions) {
        this.projectPath = options.projectPath;
        this.includeOutput = options.includeOutput ?? false;
        this.includeCodeSnippets = options.includeCodeSnippets ?? true;
    }

    /**
     * Generate markdown report from validation run
     * @param run Validation run results
     * @param outputPath Path to save markdown report
     */
    async generateMarkdownReport(run: ValidationRun, outputPath: string): Promise<void> {
        const report = this.buildReport(run);
        const markdown = this.renderMarkdown(report);

        await fs.mkdir(path.dirname(outputPath), { recursive: true });
        await fs.writeFile(outputPath, markdown, 'utf-8');
    }

    /**
     * Generate JSON report from validation run
     * @param run Validation run results
     * @param outputPath Path to save JSON report
     */
    async generateJSONReport(run: ValidationRun, outputPath: string): Promise<void> {
        const report = this.buildReport(run);

        await fs.mkdir(path.dirname(outputPath), { recursive: true });
        await fs.writeFile(outputPath, JSON.stringify(report, null, 2), 'utf-8');
    }

    /**
     * Build structured report from validation run
     */
    private buildReport(run: ValidationRun): ValidationReport {
        const metadata: ReportMetadata = {
            generatedAt: new Date().toISOString(),
            runTimestamp: run.timestamp,
            duration: run.duration,
            projectPath: this.projectPath,
        };

        const summary: ValidationSummary = {
            totalTests: run.totalTests,
            strongTests: run.strongTests,
            weakTests: run.weakTests,
            injectionFailed: run.injectionFailed,
            timeouts: run.timeouts,
            crashes: run.crashes,
            coverage: run.totalTests > 0 ? (run.strongTests / run.totalTests) * 100 : 0,
            duration: run.duration,
        };

        const weakTests = this.buildWeakTestReports(run.results);
        const strongTests = this.buildStrongTestReports(run.results);

        return {
            summary,
            weakTests,
            strongTests,
            metadata,
        };
    }

    /**
     * Build weak test reports with recommendations
     */
    private buildWeakTestReports(results: ValidationResult[]): WeakTestReport[] {
        const weakResults = results.filter(r => r.validation === 'WEAK');

        return weakResults.map(result => ({
            testId: result.testId,
            testName: result.testName,
            bugDescription: result.bugDescription,
            whyWeak: this.explainWhyWeak(result),
            recommendation: this.generateRecommendation(result),
            codeSnippet: this.includeCodeSnippets ? this.generateCodeSnippet(result) : undefined,
        }));
    }

    /**
     * Build strong test reports
     */
    private buildStrongTestReports(results: ValidationResult[]): StrongTestReport[] {
        const strongResults = results.filter(r => r.validation === 'STRONG');

        return strongResults.map(result => ({
            testId: result.testId,
            testName: result.testName,
            bugDescription: result.bugDescription,
            message: `Test successfully caught: ${result.bugDescription}`,
        }));
    }

    /**
     * Explain why a test is weak
     */
    private explainWhyWeak(result: ValidationResult): string {
        return `The test passed even though the following bug was injected: ${result.bugDescription}. ` +
            `This indicates the test is not validating this functionality properly.`;
    }

    /**
     * Generate recommendation for improving weak test
     */
    private generateRecommendation(result: ValidationResult): string {
        const bug = result.bugDescription.toLowerCase();

        // Pattern-based recommendations
        if (bug.includes('theme') && bug.includes('class')) {
            return 'Add assertion to verify theme classes are present in the DOM. ' +
                'Use expect(element).toHaveClass() to check for uk-dark or uk-light classes.';
        }

        if (bug.includes('theme') && bug.includes('localstorage')) {
            return 'Add assertion to verify theme preference is saved to localStorage. ' +
                'Use expect(localStorage.getItem("theme")).toBe("dark") to validate persistence.';
        }

        if (bug.includes('theme') && bug.includes('button')) {
            return 'Add assertion to verify theme toggle button state reflects current theme. ' +
                'Check button text, icon, or aria-label matches the theme.';
        }

        if (bug.includes('page') && bug.includes('navigation')) {
            return 'Add assertion to verify page navigation updates the DOM correctly. ' +
                'Check that the current page content is visible and other pages are hidden.';
        }

        if (bug.includes('mode') && bug.includes('reader')) {
            return 'Add assertion to verify reader mode changes are applied to the DOM. ' +
                'Check for mode-specific classes or styles on the reader container.';
        }

        if (bug.includes('search') || bug.includes('filter')) {
            return 'Add assertion to verify search/filter results are correct. ' +
                'Count visible items and ensure they match the expected filter criteria.';
        }

        if (bug.includes('sort')) {
            return 'Add assertion to verify sort order is applied correctly. ' +
                'Check that items appear in the expected order after sorting.';
        }

        if (bug.includes('progress')) {
            return 'Add assertion to verify progress updates are reflected in the UI. ' +
                'Check progress bar width, percentage text, or completion status.';
        }

        if (bug.includes('visibility') || bug.includes('display')) {
            return 'Add assertion to verify element visibility. ' +
                'Use expect(element).toBeVisible() or expect(element).toHaveCSS("display", "block").';
        }

        if (bug.includes('click') || bug.includes('event')) {
            return 'Add assertion to verify click handlers execute correctly. ' +
                'Check that the expected state change occurs after the click event.';
        }

        // Generic recommendation
        return 'Add specific assertion to verify the functionality affected by this bug. ' +
            'The test should fail when this bug is present.';
    }

    /**
     * Generate code snippet showing how to fix weak test
     */
    private generateCodeSnippet(result: ValidationResult): string | undefined {
        const bug = result.bugDescription.toLowerCase();

        if (bug.includes('theme') && bug.includes('class')) {
            return `// Add assertion to verify theme class\n` +
                `const body = page.locator('body');\n` +
                `await expect(body).toHaveClass(/uk-dark/);\n` +
                `// or\n` +
                `await expect(body).toHaveClass(/uk-light/);`;
        }

        if (bug.includes('theme') && bug.includes('localstorage')) {
            return `// Add assertion to verify localStorage persistence\n` +
                `const theme = await page.evaluate(() => localStorage.getItem('theme'));\n` +
                `expect(theme).toBe('dark'); // or 'light'`;
        }

        if (bug.includes('search') || bug.includes('filter')) {
            return `// Add assertion to verify filtered results\n` +
                `const visibleItems = page.locator('.item:visible');\n` +
                `await expect(visibleItems).toHaveCount(expectedCount);\n` +
                `// or verify specific items are visible\n` +
                `await expect(page.locator('.item:has-text("Expected")')).toBeVisible();`;
        }

        if (bug.includes('sort')) {
            return `// Add assertion to verify sort order\n` +
                `const items = await page.locator('.item .title').allTextContents();\n` +
                `expect(items).toEqual(['A', 'B', 'C']); // Expected sorted order`;
        }

        if (bug.includes('visibility')) {
            return `// Add assertion to verify element visibility\n` +
                `const element = page.locator('.target-element');\n` +
                `await expect(element).toBeVisible();\n` +
                `// or\n` +
                `await expect(element).toHaveCSS('display', 'block');`;
        }

        // No specific snippet for this case
        return undefined;
    }

    /**
     * Render markdown report
     */
    private renderMarkdown(report: ValidationReport): string {
        const lines: string[] = [];

        // Title
        lines.push('# Test Validation Report\n');

        // Metadata
        lines.push(`**Generated:** ${new Date(report.metadata.generatedAt).toLocaleString()}`);
        lines.push(`**Validation Run:** ${new Date(report.metadata.runTimestamp).toLocaleString()}`);
        lines.push(`**Duration:** ${(report.metadata.duration / 1000).toFixed(1)}s`);
        lines.push(`**Project:** ${report.metadata.projectPath}\n`);

        // Summary
        lines.push('## Summary\n');
        lines.push('| Metric | Count | Percentage |');
        lines.push('|--------|-------|------------|');
        lines.push(`| **Total Validations** | ${report.summary.totalTests} | 100.0% |`);
        lines.push(`| ✅ **STRONG Tests** | ${report.summary.strongTests} | ${((report.summary.strongTests / report.summary.totalTests) * 100).toFixed(1)}% |`);
        lines.push(`| ❌ **WEAK Tests** | ${report.summary.weakTests} | ${((report.summary.weakTests / report.summary.totalTests) * 100).toFixed(1)}% |`);

        if (report.summary.injectionFailed > 0) {
            lines.push(`| ⚠️ **Injection Failed** | ${report.summary.injectionFailed} | ${((report.summary.injectionFailed / report.summary.totalTests) * 100).toFixed(1)}% |`);
        }
        if (report.summary.timeouts > 0) {
            lines.push(`| ⏱️ **Timeouts** | ${report.summary.timeouts} | ${((report.summary.timeouts / report.summary.totalTests) * 100).toFixed(1)}% |`);
        }
        if (report.summary.crashes > 0) {
            lines.push(`| 💥 **Crashes** | ${report.summary.crashes} | ${((report.summary.crashes / report.summary.totalTests) * 100).toFixed(1)}% |`);
        }

        lines.push('');
        lines.push(`**Test Effectiveness:** ${report.summary.coverage.toFixed(1)}%\n`);

        // Goal assessment
        const goalCoverage = 90;
        if (report.summary.coverage >= goalCoverage) {
            lines.push(`✅ **Goal Achieved!** Coverage ≥ ${goalCoverage}%\n`);
        } else {
            lines.push(`⚠️ **Goal Not Met:** Target is ${goalCoverage}% coverage (${(goalCoverage - report.summary.coverage).toFixed(1)}% to go)\n`);
        }

        // Weak Tests Section
        if (report.weakTests.length > 0) {
            lines.push('## ❌ Weak Tests (Need Improvement)\n');
            lines.push(`Found ${report.weakTests.length} weak test(s) that did not catch injected bugs.\n`);

            for (let i = 0; i < report.weakTests.length; i++) {
                const test = report.weakTests[i];
                lines.push(`### ${i + 1}. ${test.testName}\n`);
                lines.push(`**Test ID:** \`${test.testId}\`\n`);
                lines.push(`**Bug Missed:** ${test.bugDescription}\n`);
                lines.push(`**Why Weak:** ${test.whyWeak}\n`);
                lines.push(`**Recommendation:** ${test.recommendation}\n`);

                if (test.codeSnippet) {
                    lines.push('**Example Fix:**\n');
                    lines.push('```typescript');
                    lines.push(test.codeSnippet);
                    lines.push('```\n');
                }

                lines.push('---\n');
            }
        }

        // Strong Tests Section
        if (report.strongTests.length > 0) {
            lines.push('## ✅ Strong Tests (Effective)\n');
            lines.push(`Found ${report.strongTests.length} strong test(s) that successfully caught injected bugs.\n`);

            // Group by test for cleaner output
            const groupedByTest = new Map<string, StrongTestReport[]>();
            for (const test of report.strongTests) {
                const existing = groupedByTest.get(test.testName) || [];
                existing.push(test);
                groupedByTest.set(test.testName, existing);
            }

            for (const [testName, tests] of groupedByTest) {
                lines.push(`### ${testName}\n`);
                lines.push(`**Test ID:** \`${tests[0].testId}\`\n`);
                lines.push('**Bugs Caught:**\n');
                for (const test of tests) {
                    lines.push(`- ${test.bugDescription}`);
                }
                lines.push('');
            }
        }

        // Best Practices Section
        lines.push('## 📚 Best Practices\n');
        lines.push('Based on this validation run, follow these best practices:\n');

        if (report.weakTests.length > 0) {
            lines.push('### Common Issues Found\n');

            const themeIssues = report.weakTests.filter(t => t.bugDescription.toLowerCase().includes('theme'));
            const searchIssues = report.weakTests.filter(t =>
                t.bugDescription.toLowerCase().includes('search') ||
                t.bugDescription.toLowerCase().includes('filter')
            );
            const navigationIssues = report.weakTests.filter(t =>
                t.bugDescription.toLowerCase().includes('navigation') ||
                t.bugDescription.toLowerCase().includes('page')
            );

            if (themeIssues.length > 0) {
                lines.push(`- **Theme Testing (${themeIssues.length} weak tests):** Always verify both DOM state (classes) and persistence (localStorage) when testing theme toggles.`);
            }
            if (searchIssues.length > 0) {
                lines.push(`- **Search/Filter Testing (${searchIssues.length} weak tests):** Verify result counts and content, not just UI interactions.`);
            }
            if (navigationIssues.length > 0) {
                lines.push(`- **Navigation Testing (${navigationIssues.length} weak tests):** Check both visibility state and content updates after navigation.`);
            }

            lines.push('');
        }

        lines.push('### General Guidelines\n');
        lines.push('1. **Test Outcomes, Not Actions:** Verify the result of user actions, not just that actions were performed.');
        lines.push('2. **Assert Specific State:** Use precise assertions (toHaveClass, toHaveText) rather than generic checks.');
        lines.push('3. **Validate Persistence:** For features with state persistence, verify storage mechanisms.');
        lines.push('4. **Check Multiple Aspects:** Test both UI state and underlying data changes.');
        lines.push('5. **Use Meaningful Waits:** Wait for specific conditions rather than arbitrary timeouts.\n');

        // Footer
        lines.push('---\n');
        lines.push('*Report generated by Test Validation Framework*');

        return lines.join('\n');
    }

    /**
     * Get summary statistics as formatted string
     */
    formatSummary(summary: ValidationSummary): string {
        return `Total: ${summary.totalTests} | ` +
            `STRONG: ${summary.strongTests} (${((summary.strongTests / summary.totalTests) * 100).toFixed(1)}%) | ` +
            `WEAK: ${summary.weakTests} (${((summary.weakTests / summary.totalTests) * 100).toFixed(1)}%) | ` +
            `Coverage: ${summary.coverage.toFixed(1)}%`;
    }

    /**
     * Generate quick summary for console output
     */
    generateQuickSummary(run: ValidationRun): string {
        const lines: string[] = [];

        lines.push('Validation Summary:');
        lines.push(`  Total: ${run.totalTests}`);
        lines.push(`  STRONG: ${run.strongTests} (${((run.strongTests / run.totalTests) * 100).toFixed(1)}%)`);
        lines.push(`  WEAK: ${run.weakTests} (${((run.weakTests / run.totalTests) * 100).toFixed(1)}%)`);

        if (run.injectionFailed > 0) {
            lines.push(`  Injection Failed: ${run.injectionFailed}`);
        }
        if (run.timeouts > 0) {
            lines.push(`  Timeouts: ${run.timeouts}`);
        }
        if (run.crashes > 0) {
            lines.push(`  Crashes: ${run.crashes}`);
        }

        lines.push(`  Coverage: ${((run.strongTests / run.totalTests) * 100).toFixed(1)}%`);
        lines.push(`  Duration: ${(run.duration / 1000).toFixed(1)}s`);

        return lines.join('\n');
    }
}
