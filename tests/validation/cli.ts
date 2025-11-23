#!/usr/bin/env node

/**
 * Test Validation Framework - CLI Entry Point
 * Command-line interface for running validation and generating reports
 */

import * as path from 'path';
import * as fs from 'fs/promises';
import chalk from 'chalk';
import minimist from 'minimist';
import { ValidationOrchestrator } from './core/orchestrator.js';
import { ReportGenerator } from './core/report-generator.js';
import { ValidationRun } from './types/index.js';

/**
 * CLI Configuration
 */
interface CLIConfig {
    command: 'validate' | 'report' | 'help';
    testId?: string;
    catalogFile?: string;
    outputDir?: string;
    reportFile?: string;
    dryRun?: boolean;
    verbose?: boolean;
    timeout?: number;
}

/**
 * Parse command-line arguments
 */
function parseArgs(): CLIConfig {
    const args = minimist(process.argv.slice(2), {
        string: ['test', 'catalog', 'output', 'report', 'timeout'],
        boolean: ['dry-run', 'verbose', 'help', 'report-only'],
        alias: {
            t: 'test',
            c: 'catalog',
            o: 'output',
            r: 'report',
            d: 'dry-run',
            v: 'verbose',
            h: 'help',
        },
        default: {
            catalog: 'tests/validation/bug-catalog.yaml',
            output: '.validation-results',
            timeout: 60000,
        },
    });

    // Determine command
    let command: 'validate' | 'report' | 'help' = 'validate';
    if (args.help) {
        command = 'help';
    } else if (args['report-only'] || args.report) {
        command = 'report';
    }

    return {
        command,
        testId: args.test,
        catalogFile: args.catalog,
        outputDir: args.output,
        reportFile: args.report,
        dryRun: args['dry-run'],
        verbose: args.verbose,
        timeout: parseInt(args.timeout, 10),
    };
}

/**
 * Display help text
 */
function displayHelp(): void {
    console.log(chalk.bold('\n🧪 Test Validation Framework\n'));
    console.log('Usage: node cli.js [options]\n');

    console.log(chalk.bold('Commands:'));
    console.log('  validate          Run validation on all tests (default)');
    console.log('  --report-only     Generate report from existing results\n');

    console.log(chalk.bold('Options:'));
    console.log('  -t, --test <id>       Validate specific test by ID');
    console.log('  -c, --catalog <file>  Bug catalog YAML file (default: tests/validation/bug-catalog.yaml)');
    console.log('  -o, --output <dir>    Output directory for results (default: .validation-results)');
    console.log('  -r, --report <file>   Existing validation results JSON for report generation');
    console.log('  -d, --dry-run         Show what would be validated without running');
    console.log('  -v, --verbose         Show detailed output');
    console.log('  --timeout <ms>        Test timeout in milliseconds (default: 60000)');
    console.log('  -h, --help            Show this help\n');

    console.log(chalk.bold('Examples:'));
    console.log('  # Validate all tests');
    console.log(chalk.gray('  $ node cli.js\n'));

    console.log('  # Validate specific test');
    console.log(chalk.gray('  $ node cli.js --test "tests/integration/theme.spec.ts:10:5"\n'));

    console.log('  # Dry run to see what would be validated');
    console.log(chalk.gray('  $ node cli.js --dry-run\n'));

    console.log('  # Generate report from existing results');
    console.log(chalk.gray('  $ node cli.js --report-only --report .validation-results/run-123/results.json\n'));

    console.log('  # Verbose output for debugging');
    console.log(chalk.gray('  $ node cli.js --verbose\n'));
}

/**
 * Run validation
 */
async function runValidation(config: CLIConfig): Promise<void> {
    try {
        // Get project root (parent of tests/validation)
        const projectPath = path.resolve(process.cwd(), '../..');
        const catalogPath = path.resolve(process.cwd(), config.catalogFile!);

        if (config.verbose) {
            console.log(chalk.gray(`Project path: ${projectPath}`));
            console.log(chalk.gray(`Catalog path: ${catalogPath}`));
        }

        // Check catalog exists
        try {
            await fs.access(catalogPath);
        } catch {
            console.error(chalk.red(`\n✗ Error: Catalog file not found: ${catalogPath}\n`));
            process.exit(1);
        }

        // Dry run mode
        if (config.dryRun) {
            console.log(chalk.yellow('\n🔍 Dry Run Mode - No validation will be performed\n'));
            await displayDryRun(catalogPath);
            return;
        }

        // Create orchestrator
        const orchestrator = new ValidationOrchestrator({
            projectPath,
            catalogPath,
            testsPath: path.resolve(projectPath, 'tests'),
            timeout: config.timeout,
            verbose: config.verbose,
        });

        // Run validation
        let results: ValidationRun;

        if (config.testId) {
            console.log(chalk.cyan(`\nValidating single test: ${config.testId}\n`));
            // For single test validation, we need to load catalog and find the test
            // This is a simplified implementation - full implementation would parse testId
            console.log(chalk.yellow('Note: Single test validation not fully implemented yet.'));
            console.log(chalk.yellow('Running full validation instead...\n'));
            results = await orchestrator.validateAll();
        } else {
            results = await orchestrator.validateAll();
        }

        // Save results
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
        const outputDir = path.resolve(process.cwd(), config.outputDir!, `run-${timestamp}`);
        const resultsPath = path.join(outputDir, 'results.json');

        await orchestrator.saveResults(results, resultsPath);

        if (config.verbose) {
            console.log(chalk.gray(`\nResults saved to: ${resultsPath}`));
        }

        // Generate reports
        const reportGen = new ReportGenerator({
            projectPath,
            includeOutput: config.verbose,
            includeCodeSnippets: true,
        });

        const markdownPath = path.join(outputDir, 'report.md');
        const jsonPath = path.join(outputDir, 'report.json');

        await reportGen.generateMarkdownReport(results, markdownPath);
        await reportGen.generateJSONReport(results, jsonPath);

        console.log(chalk.green(`\n✓ Reports generated:`));
        console.log(chalk.gray(`  Markdown: ${markdownPath}`));
        console.log(chalk.gray(`  JSON:     ${jsonPath}`));
        console.log(chalk.gray(`  Results:  ${resultsPath}\n`));

        // Exit with error code if there are weak tests
        if (results.weakTests > 0) {
            console.log(chalk.yellow(`⚠ Found ${results.weakTests} weak test(s). See report for details.\n`));
            process.exit(1);
        } else {
            console.log(chalk.green(`🎉 All tests are strong!\n`));
            process.exit(0);
        }
    } catch (error) {
        console.error(chalk.red(`\n✗ Validation failed: ${error}\n`));
        if (config.verbose && error instanceof Error) {
            console.error(chalk.gray(error.stack));
        }
        process.exit(1);
    }
}

/**
 * Generate report from existing results
 */
async function generateReport(config: CLIConfig): Promise<void> {
    try {
        if (!config.reportFile) {
            console.error(chalk.red('\n✗ Error: --report <file> is required for report generation\n'));
            console.log(chalk.gray('Example: node cli.js --report-only --report .validation-results/run-123/results.json\n'));
            process.exit(1);
        }

        const resultsPath = path.resolve(process.cwd(), config.reportFile);

        // Load results
        try {
            await fs.access(resultsPath);
        } catch {
            console.error(chalk.red(`\n✗ Error: Results file not found: ${resultsPath}\n`));
            process.exit(1);
        }

        const content = await fs.readFile(resultsPath, 'utf-8');
        const results: ValidationRun = JSON.parse(content);

        console.log(chalk.cyan('\n📊 Generating report from existing results...\n'));

        // Get project root
        const projectPath = path.resolve(process.cwd(), '../..');

        // Generate reports
        const reportGen = new ReportGenerator({
            projectPath,
            includeOutput: config.verbose,
            includeCodeSnippets: true,
        });

        const outputDir = path.dirname(resultsPath);
        const markdownPath = path.join(outputDir, 'report.md');
        const jsonPath = path.join(outputDir, 'report.json');

        await reportGen.generateMarkdownReport(results, markdownPath);
        await reportGen.generateJSONReport(results, jsonPath);

        console.log(chalk.green('✓ Reports generated:'));
        console.log(chalk.gray(`  Markdown: ${markdownPath}`));
        console.log(chalk.gray(`  JSON:     ${jsonPath}\n`));

        // Display summary
        console.log(reportGen.generateQuickSummary(results));
        console.log('');

        process.exit(0);
    } catch (error) {
        console.error(chalk.red(`\n✗ Report generation failed: ${error}\n`));
        if (config.verbose && error instanceof Error) {
            console.error(chalk.gray(error.stack));
        }
        process.exit(1);
    }
}

/**
 * Display dry run information
 */
async function displayDryRun(catalogPath: string): Promise<void> {
    try {
        const yaml = await import('yaml');
        const content = await fs.readFile(catalogPath, 'utf-8');
        const catalog = yaml.parse(content);

        console.log(chalk.bold('Validation Plan:\n'));

        console.log(`Catalog: ${path.basename(catalogPath)}`);
        console.log(`Tests: ${catalog.tests.length}`);

        let totalBugs = 0;
        for (const test of catalog.tests) {
            totalBugs += test.bugs.length;
        }

        console.log(`Total bug injections: ${totalBugs}\n`);

        console.log(chalk.bold('Test Breakdown:\n'));

        for (let i = 0; i < Math.min(5, catalog.tests.length); i++) {
            const test = catalog.tests[i];
            console.log(chalk.cyan(`${i + 1}. ${test.testName}`));
            console.log(chalk.gray(`   File: ${test.testFile}`));
            console.log(chalk.gray(`   Bugs: ${test.bugs.length}`));

            for (const bug of test.bugs.slice(0, 2)) {
                console.log(chalk.gray(`     - ${bug.description}`));
            }

            if (test.bugs.length > 2) {
                console.log(chalk.gray(`     ... and ${test.bugs.length - 2} more`));
            }

            console.log('');
        }

        if (catalog.tests.length > 5) {
            console.log(chalk.gray(`... and ${catalog.tests.length - 5} more tests\n`));
        }

        console.log(chalk.yellow('Run without --dry-run to execute validation.\n'));
    } catch (error) {
        console.error(chalk.red(`Failed to parse catalog: ${error}\n`));
        process.exit(1);
    }
}

/**
 * Main CLI entry point
 */
async function main(): Promise<void> {
    const config = parseArgs();

    switch (config.command) {
        case 'help':
            displayHelp();
            break;

        case 'validate':
            await runValidation(config);
            break;

        case 'report':
            await generateReport(config);
            break;

        default:
            console.error(chalk.red(`Unknown command: ${config.command}\n`));
            displayHelp();
            process.exit(1);
    }
}

// Run CLI
main().catch(error => {
    console.error(chalk.red(`\n✗ Fatal error: ${error}\n`));
    process.exit(1);
});
