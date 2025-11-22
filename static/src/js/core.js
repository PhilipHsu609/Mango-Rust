// ============================================================================
// Core JavaScript - Shared utilities loaded on all pages
// Theme management, localStorage helpers, initialization
// ============================================================================

// ------------------------------------------------------------------------
// Theme Management
// ------------------------------------------------------------------------

function getTheme() {
    return localStorage.getItem('theme') || detectSystemTheme();
}

function detectSystemTheme() {
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
        return 'dark';
    }
    return 'light';
}

function applyTheme(theme) {
    if (theme === 'dark') {
        document.body.classList.add('uk-light');
    } else {
        document.body.classList.remove('uk-light');
    }
    localStorage.setItem('theme', theme);
}

function toggleTheme() {
    const currentTheme = getTheme();
    const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
    applyTheme(newTheme);
}

// Apply theme before DOM renders (prevent FOUC)
(function() {
    const theme = getTheme();
    if (theme === 'dark') {
        document.documentElement.classList.add('uk-light');
        if (document.body) {
            document.body.classList.add('uk-light');
        }
    }
})();

// ------------------------------------------------------------------------
// localStorage Helpers
// ------------------------------------------------------------------------

function savePreference(key, value) {
    try {
        localStorage.setItem(key, JSON.stringify(value));
    } catch (e) {
        console.error('Failed to save preference:', e);
    }
}

function loadPreference(key, defaultValue) {
    try {
        const value = localStorage.getItem(key);
        return value ? JSON.parse(value) : defaultValue;
    } catch (e) {
        console.error('Failed to load preference:', e);
        return defaultValue;
    }
}

// ------------------------------------------------------------------------
// Initialization
// ------------------------------------------------------------------------

document.addEventListener('DOMContentLoaded', () => {
    // Apply theme on page load
    applyTheme(getTheme());

    // Listen for system theme changes
    if (window.matchMedia) {
        window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
            // Only auto-switch if user hasn't manually set a preference
            if (!localStorage.getItem('theme')) {
                applyTheme(e.matches ? 'dark' : 'light');
            }
        });
    }
});
