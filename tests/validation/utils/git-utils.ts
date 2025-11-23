/**
 * Git utilities for branch management during bug injection
 * Provides safe git operations with clean state verification
 */

import simpleGit, { SimpleGit } from 'simple-git';
import { BranchInfo } from '../types/index.js';

/**
 * Initialize git instance for project root
 */
export function createGit(projectPath: string): SimpleGit {
    return simpleGit(projectPath);
}

/**
 * Get current branch name
 * @throws Error if git operation fails
 */
export async function getCurrentBranch(git: SimpleGit): Promise<string> {
    try {
        const branch = await git.revparse(['--abbrev-ref', 'HEAD']);
        return branch.trim();
    } catch (error) {
        throw new Error(`Failed to get current branch: ${error}`);
    }
}

/**
 * Check if repository has uncommitted changes
 * @returns true if repository is clean (no uncommitted changes)
 */
export async function checkCleanState(git: SimpleGit): Promise<boolean> {
    try {
        const status = await git.status();

        // Check for uncommitted changes
        const isDirty = status.files.length > 0;

        if (isDirty) {
            return false;
        }

        return true;
    } catch (error) {
        throw new Error(`Failed to check repository state: ${error}`);
    }
}

/**
 * Create a new branch for bug injection
 * @param git SimpleGit instance
 * @param branchName Branch name to create
 * @throws Error if branch already exists or creation fails
 */
export async function createBranch(git: SimpleGit, branchName: string): Promise<void> {
    try {
        // Check if branch already exists
        const branches = await git.branchLocal();
        if (branches.all.includes(branchName)) {
            // Delete existing branch first
            await git.deleteLocalBranch(branchName, true);
        }

        // Create and checkout new branch
        await git.checkoutLocalBranch(branchName);
    } catch (error) {
        throw new Error(`Failed to create branch '${branchName}': ${error}`);
    }
}

/**
 * Switch to a different branch
 * @param git SimpleGit instance
 * @param branchName Branch to switch to
 * @throws Error if branch doesn't exist or checkout fails
 */
export async function switchBranch(git: SimpleGit, branchName: string): Promise<void> {
    try {
        await git.checkout(branchName);
    } catch (error) {
        throw new Error(`Failed to switch to branch '${branchName}': ${error}`);
    }
}

/**
 * Delete a local branch
 * @param git SimpleGit instance
 * @param branchName Branch to delete
 * @param force Force delete even if not merged
 * @throws Error if deletion fails
 */
export async function deleteBranch(
    git: SimpleGit,
    branchName: string,
    force: boolean = true
): Promise<void> {
    try {
        await git.deleteLocalBranch(branchName, force);
    } catch (error) {
        // Ignore error if branch doesn't exist
        const errorMsg = String(error);
        if (!errorMsg.includes('not found')) {
            throw new Error(`Failed to delete branch '${branchName}': ${error}`);
        }
    }
}

/**
 * Create a validation branch for bug injection
 * @param git SimpleGit instance
 * @param bugId Bug identifier
 * @param testId Test identifier
 * @returns BranchInfo with branch details
 */
export async function createValidationBranch(
    git: SimpleGit,
    bugId: string,
    testId: string
): Promise<BranchInfo> {
    // Ensure repository is clean
    const isClean = await checkCleanState(git);
    if (!isClean) {
        throw new Error(
            'Repository has uncommitted changes. Please commit or stash your changes before running validation.'
        );
    }

    // Get current branch
    const originalBranch = await getCurrentBranch(git);

    // Create branch name: test-validation/{testId}/{bugId}
    const safeBugId = bugId.replace(/[^a-zA-Z0-9-]/g, '-');
    const safeTestId = testId.replace(/[^a-zA-Z0-9-]/g, '-');
    const branchName = `test-validation/${safeTestId}/${safeBugId}`;

    // Create and checkout branch
    await createBranch(git, branchName);

    return {
        branchName,
        originalBranch,
        bugId,
        timestamp: Date.now(),
    };
}

/**
 * Cleanup validation branch and return to original branch
 * @param git SimpleGit instance
 * @param branchInfo Branch information from createValidationBranch
 */
export async function cleanupValidationBranch(
    git: SimpleGit,
    branchInfo: BranchInfo
): Promise<void> {
    try {
        // Discard all uncommitted changes in the validation branch
        await git.reset(['--hard']);

        // Switch back to original branch
        await switchBranch(git, branchInfo.originalBranch);

        // Delete validation branch
        await deleteBranch(git, branchInfo.branchName, true);
    } catch (error) {
        throw new Error(`Failed to cleanup validation branch: ${error}`);
    }
}

/**
 * Verify repository is in clean state before validation
 * Throws descriptive error if repository is dirty
 */
export async function verifyCleanRepository(git: SimpleGit): Promise<void> {
    const isClean = await checkCleanState(git);

    if (!isClean) {
        const status = await git.status();
        const fileList = status.files.map(f => `  - ${f.path} (${f.working_dir})`).join('\n');

        throw new Error(
            `Cannot run validation with uncommitted changes:\n${fileList}\n\n` +
            'Please commit or stash your changes before running validation.'
        );
    }
}

/**
 * Get list of all validation branches
 * @returns Array of validation branch names
 */
export async function getValidationBranches(git: SimpleGit): Promise<string[]> {
    try {
        const branches = await git.branchLocal();
        return branches.all.filter(branch => branch.startsWith('test-validation/'));
    } catch (error) {
        throw new Error(`Failed to get validation branches: ${error}`);
    }
}

/**
 * Cleanup all validation branches
 * Useful for cleaning up after interrupted validation runs
 */
export async function cleanupAllValidationBranches(git: SimpleGit): Promise<number> {
    const validationBranches = await getValidationBranches(git);

    // Get current branch
    const currentBranch = await getCurrentBranch(git);

    // If we're on a validation branch, switch to main/master first
    if (currentBranch.startsWith('test-validation/')) {
        const branches = await git.branchLocal();
        const targetBranch = branches.all.includes('main') ? 'main' : 'master';
        await switchBranch(git, targetBranch);
    }

    // Delete all validation branches
    for (const branch of validationBranches) {
        await deleteBranch(git, branch, true);
    }

    return validationBranches.length;
}
