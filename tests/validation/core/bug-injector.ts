/**
 * Bug Injector - Applies code mutations to test bug detection
 * Handles file modification with exact string matching and validation
 */

import * as fs from 'fs/promises';
import * as path from 'path';
import { SimpleGit } from 'simple-git';
import { BugDefinition, BranchInfo } from '../types/index.js';
import { createValidationBranch, cleanupValidationBranch } from '../utils/git-utils.js';

/**
 * Bug Injector class for managing code mutations
 */
export class BugInjector {
    private git: SimpleGit;
    private projectPath: string;

    constructor(git: SimpleGit, projectPath: string) {
        this.git = git;
        this.projectPath = projectPath;
    }

    /**
     * Create a bug branch and apply the bug change
     * @param bug Bug definition to inject
     * @param testId Test identifier for branch naming
     * @returns BranchInfo for cleanup
     */
    async injectBug(bug: BugDefinition, testId: string): Promise<BranchInfo> {
        // Create validation branch
        const branchInfo = await createValidationBranch(this.git, bug.id, testId);

        try {
            // Apply the bug change
            await this.applyBugChange(bug);

            // Validate that the change was actually applied
            await this.validateBugApplied(bug);

            return branchInfo;
        } catch (error) {
            // If bug injection fails, cleanup branch and re-throw
            await cleanupValidationBranch(this.git, branchInfo);
            throw error;
        }
    }

    /**
     * Revert bug injection by cleaning up the branch
     * @param branchInfo Branch information from injectBug
     */
    async revertBug(branchInfo: BranchInfo): Promise<void> {
        await cleanupValidationBranch(this.git, branchInfo);
    }

    /**
     * Apply bug change to the target file
     * @param bug Bug definition
     */
    private async applyBugChange(bug: BugDefinition): Promise<void> {
        const filePath = path.join(this.projectPath, bug.targetFile);

        // Verify file exists
        try {
            await fs.access(filePath);
        } catch {
            throw new Error(`Target file not found: ${bug.targetFile}`);
        }

        // Read file contents
        const content = await fs.readFile(filePath, 'utf-8');

        // Apply bug based on strategy
        let newContent: string;

        switch (bug.strategy) {
            case 'replace':
                newContent = this.applyReplace(content, bug);
                break;

            case 'delete':
                newContent = this.applyDelete(content, bug);
                break;

            case 'comment':
                newContent = this.applyComment(content, bug);
                break;

            default:
                throw new Error(`Unknown bug strategy: ${(bug as any).strategy}`);
        }

        // Write modified content back to file
        await fs.writeFile(filePath, newContent, 'utf-8');
    }

    /**
     * Apply 'replace' strategy
     */
    private applyReplace(content: string, bug: BugDefinition): string {
        if (!bug.replace) {
            throw new Error(`Bug ${bug.id}: 'replace' strategy requires 'replace' field`);
        }

        // Check if pattern exists
        if (!content.includes(bug.find)) {
            throw new Error(
                `Bug ${bug.id}: Pattern not found in ${bug.targetFile}\n` +
                `Looking for: ${bug.find.substring(0, 100)}...`
            );
        }

        // Check if pattern is unique
        const occurrences = content.split(bug.find).length - 1;
        if (occurrences > 1) {
            throw new Error(
                `Bug ${bug.id}: Pattern appears ${occurrences} times in ${bug.targetFile} (must be unique)\n` +
                `Pattern: ${bug.find.substring(0, 100)}...`
            );
        }

        return content.replace(bug.find, bug.replace);
    }

    /**
     * Apply 'delete' strategy
     */
    private applyDelete(content: string, bug: BugDefinition): string {
        // Check if pattern exists
        if (!content.includes(bug.find)) {
            throw new Error(
                `Bug ${bug.id}: Pattern not found in ${bug.targetFile}\n` +
                `Looking for: ${bug.find.substring(0, 100)}...`
            );
        }

        // Remove the matching text
        return content.replace(bug.find, '');
    }

    /**
     * Apply 'comment' strategy
     * Comments out matching lines (works for JS/TS/Rust/HTML)
     */
    private applyComment(content: string, bug: BugDefinition): string {
        // Check if pattern exists
        if (!content.includes(bug.find)) {
            throw new Error(
                `Bug ${bug.id}: Pattern not found in ${bug.targetFile}\n` +
                `Looking for: ${bug.find.substring(0, 100)}...`
            );
        }

        // Determine comment syntax based on file extension
        const ext = path.extname(bug.targetFile);
        let commentPrefix: string;

        if (ext === '.rs') {
            commentPrefix = '//';
        } else if (ext === '.js' || ext === '.ts') {
            commentPrefix = '//';
        } else if (ext === '.html') {
            // For HTML, use <!-- ... -->
            return content.replace(bug.find, `<!-- ${bug.find} -->`);
        } else {
            // Default to // for unknown extensions
            commentPrefix = '//';
        }

        // Comment out the pattern
        return content.replace(bug.find, `${commentPrefix} ${bug.find}`);
    }

    /**
     * Validate that the bug was actually applied
     * Checks that the file content changed as expected
     */
    private async validateBugApplied(bug: BugDefinition): Promise<void> {
        const filePath = path.join(this.projectPath, bug.targetFile);
        const content = await fs.readFile(filePath, 'utf-8');

        // For 'replace' and 'delete', verify pattern is gone
        if (bug.strategy === 'replace' || bug.strategy === 'delete') {
            if (content.includes(bug.find)) {
                throw new Error(
                    `Bug ${bug.id}: Validation failed - pattern still present after ${bug.strategy}\n` +
                    `This likely means the find/replace didn't work as expected.`
                );
            }
        }

        // For 'replace', verify replacement is present
        if (bug.strategy === 'replace' && bug.replace) {
            if (!content.includes(bug.replace)) {
                throw new Error(
                    `Bug ${bug.id}: Validation failed - replacement text not found\n` +
                    `Expected to find: ${bug.replace.substring(0, 100)}...`
                );
            }
        }

        // For 'comment', verify pattern is still there but commented
        if (bug.strategy === 'comment') {
            const ext = path.extname(bug.targetFile);
            let commentedVersion: string;

            if (ext === '.html') {
                commentedVersion = `<!-- ${bug.find} -->`;
            } else {
                commentedVersion = `// ${bug.find}`;
            }

            if (!content.includes(commentedVersion) && !content.includes(bug.find)) {
                throw new Error(
                    `Bug ${bug.id}: Validation failed - pattern not found (commented or uncommented)`
                );
            }
        }
    }

    /**
     * Get stats about a bug injection (for reporting)
     */
    async getBugStats(bug: BugDefinition): Promise<{
        targetFile: string;
        strategy: string;
        patternLength: number;
        fileExists: boolean;
    }> {
        const filePath = path.join(this.projectPath, bug.targetFile);

        let fileExists = false;
        try {
            await fs.access(filePath);
            fileExists = true;
        } catch {
            // File doesn't exist
        }

        return {
            targetFile: bug.targetFile,
            strategy: bug.strategy,
            patternLength: bug.find.length,
            fileExists,
        };
    }
}
