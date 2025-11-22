/**
 * Theme management for Mango-Rust
 * Supports light, dark, and system themes
 */

// Check if the system setting prefers dark theme
const prefersDarkMode = () => {
    return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
};

// Check if a theme setting is valid
const validThemeSetting = (theme) => {
    return ['dark', 'light', 'system'].includes(theme);
};

// Load theme setting from localStorage (defaults to 'system')
const loadThemeSetting = () => {
    let setting = localStorage.getItem('theme');
    if (!setting || !validThemeSetting(setting)) {
        setting = 'system';
    }
    return setting;
};

// Get the actual theme to apply (resolves 'system' to 'light' or 'dark')
const loadTheme = () => {
    let setting = loadThemeSetting();
    if (setting === 'system') {
        setting = prefersDarkMode() ? 'dark' : 'light';
    }
    return setting;
};

// Save theme setting to localStorage
const saveThemeSetting = (setting) => {
    if (!validThemeSetting(setting)) {
        setting = 'system';
    }
    localStorage.setItem('theme', setting);
};

// Toggle between light and dark themes (simplified, matching original Mango)
const toggleTheme = () => {
    const theme = loadTheme();
    const newTheme = theme === 'dark' ? 'light' : 'dark';
    saveThemeSetting(newTheme);
    applyTheme(newTheme);
};

// Apply the theme to the page
const applyTheme = (theme) => {
    if (!theme) {
        theme = loadTheme();
    }

    const html = document.documentElement;
    const body = document.body;

    if (theme === 'dark') {
        html.style.background = 'rgb(20, 20, 20)';
        body.classList.add('uk-light');
        body.style.background = 'rgb(20, 20, 20)';
    } else {
        html.style.background = '';
        body.classList.remove('uk-light');
        body.style.background = '';
    }
};

// Initialize theme before DOM loads to prevent flash
applyTheme();

// Wait for DOM to be ready
document.addEventListener('DOMContentLoaded', () => {
    // Apply theme again (for reader page)
    applyTheme();

    // Listen for system theme changes
    if (window.matchMedia) {
        window.matchMedia('(prefers-color-scheme: dark)')
            .addEventListener('change', (event) => {
                if (loadThemeSetting() === 'system') {
                    applyTheme(event.matches ? 'dark' : 'light');
                }
            });
    }
});
