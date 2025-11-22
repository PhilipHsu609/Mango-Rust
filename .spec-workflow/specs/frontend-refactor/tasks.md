# Tasks Document: Frontend Refactoring

## Task Breakdown

### Phase 1: Setup Build System

- [x] 1.1. Create directory structure for frontend source files
  - Files: `static/src/css/`, `static/src/js/`, `static/dist/css/`, `static/dist/js/`
  - Create directory hierarchy for organized source files
  - Add to .gitignore: `static/dist/`, `node_modules/`
  - Purpose: Establish clean file organization for source and compiled assets
  - _Leverage: Original Mango's `Mango/public/` structure_
  - _Requirements: REQ-4, REQ-10_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: DevOps Engineer specializing in build systems and project structure | Task: Create comprehensive directory structure for frontend assets following requirements REQ-4 and REQ-10, establishing proper separation between source (`static/src/`) and compiled output (`static/dist/`), update .gitignore to exclude compiled assets | Restrictions: Do not delete existing static files, maintain backward compatibility during transition, follow kebab-case naming | _Leverage: Review original Mango structure in Mango/public/ for patterns_ | Success: Directories created successfully, .gitignore updated, existing static files preserved | Instructions: Before starting, mark this task as in-progress in tasks.md by changing `[ ]` to `[-]`. After completing the implementation and testing, use the log-implementation tool with comprehensive artifacts (files created, directory structure). Finally, mark task as complete `[x]` in tasks.md._

- [x] 1.2. Install LESS compiler and create build scripts
  - Files: `build-css.sh`, `watch-css.sh`, `package.json`
  - Install lessc via npm, create build and watch scripts
  - Document prerequisites in README.md
  - Purpose: Enable LESS to CSS compilation in development and production
  - _Leverage: Original Mango uses LESS (see Mango/public/css/mango.less)_
  - _Requirements: REQ-4_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Build Engineer with expertise in asset compilation and scripting | Task: Install LESS compiler globally or locally and create build scripts (build-css.sh for production, watch-css.sh for development) following requirement REQ-4, add npm scripts if using package.json | Restrictions: Scripts must work on both Linux and macOS, must handle errors gracefully, watch mode must not block terminal | _Leverage: Review Mango's LESS files for compilation patterns_ | Success: LESS compiler installed successfully, build-css.sh compiles CSS without errors, watch-css.sh auto-compiles on file changes, README.md documents setup steps | Instructions: Before starting, mark this task as in-progress in tasks.md. After implementation, use log-implementation tool to document the build system setup (scripts created, commands, dependencies). Mark complete in tasks.md._

- [x] 1.3. Test build system with simple LESS file
  - Files: `static/src/css/test.less`, compile to `static/dist/css/test.css`
  - Create minimal LESS file with variables and nesting
  - Run build-css.sh and verify output
  - Purpose: Validate build system works before migrating real code
  - _Leverage: LESS syntax examples from Mango/public/css/_
  - _Requirements: REQ-4_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Frontend Developer with LESS expertise | Task: Create test LESS file with variables and nesting following requirement REQ-4, compile it using build-css.sh, verify minification and source maps work correctly | Restrictions: Use simple LESS features only (variables, nesting, imports), must compile without warnings or errors | _Leverage: Check Mango/public/css/mango.less for LESS patterns to test_ | Success: Test LESS file compiles successfully, minified CSS output is correct, source maps are generated, watch mode detects changes | Instructions: Mark in-progress in tasks.md, test thoroughly, log implementation with build system verification results, mark complete._

### Phase 2: Extract Base CSS from Templates

- [x] 2.1. Create LESS variables file
  - Files: `static/src/css/_variables.less`
  - Extract colors, sizes, breakpoints from base.html
  - Define consistent variable naming convention
  - Purpose: Establish single source of truth for design tokens
  - _Leverage: Original Mango's color scheme and breakpoints_
  - _Requirements: REQ-1, REQ-10_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Frontend Developer specializing in design systems and LESS | Task: Create comprehensive variables file extracting all colors, sizes, and breakpoints from templates/base.html inline CSS (lines 10-318) following requirements REQ-1 and REQ-10 | Restrictions: Use kebab-case for variable names (e.g., @primary-color), organize by category (colors, sizes, breakpoints), document each variable's purpose | _Leverage: Review Mango's CSS for color consistency_ | Success: All design tokens extracted to variables, clear organization by category, consistent naming convention, documented | Instructions: Mark in-progress, extract carefully from base.html, log implementation with all variables defined, mark complete._

- [x] 2.2. Create base layout LESS file
  - Files: `static/src/css/base.less`
  - Extract global styles from base.html (html, body, main-content)
  - Import variables, use LESS features for organization
  - Purpose: Establish core layout styles in proper stylesheet
  - _Leverage: Base layout patterns from base.html lines 10-74_
  - _Requirements: REQ-1_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CSS Architect specializing in layout systems | Task: Extract base layout styles from templates/base.html inline CSS (html, body, main-content sections) to static/src/css/base.less following requirement REQ-1, import variables.less, use LESS nesting appropriately | Restrictions: Extract only layout/typography styles, not component-specific or dark theme styles, maintain exact visual appearance | _Leverage: base.html lines 10-74 for layout structure_ | Success: Base layout styles extracted completely, uses variables for colors/sizes, compiles without errors, pages render identically | Instructions: Mark in-progress, test visual parity with Playwright, log with CSS structure details, mark complete._

- [x] 2.3. Create navigation component LESS file
  - Files: `static/src/css/components/_nav.less`
  - Extract navigation styles from base.html (navbar, hamburger, offcanvas)
  - Use LESS nesting for clear component structure
  - Purpose: Isolate navigation styles into dedicated component file
  - _Leverage: Navigation styles from base.html lines 27-108_
  - _Requirements: REQ-1, REQ-10_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Component CSS Developer | Task: Extract navigation component styles from templates/base.html (lines 27-108) to static/src/css/components/_nav.less following requirements REQ-1 and REQ-10, use LESS nesting for navbar structure | Restrictions: Extract only navigation-related styles (navbar, hamburger, offcanvas), use BEM or similar naming for classes, maintain mobile responsiveness | _Leverage: base.html navigation HTML structure_ | Success: Navigation styles fully extracted, properly nested LESS, mobile hamburger works identically, offcanvas slides correctly | Instructions: Mark in-progress, test mobile and desktop nav, log with component structure, mark complete._

- [x] 2.4. Create main LESS entry point
  - Files: `static/src/css/main.less`
  - Import all LESS files in correct order
  - Configure compilation to produce main.css
  - Purpose: Single entry point for CSS compilation
  - _Leverage: Original Mango's import pattern_
  - _Requirements: REQ-1, REQ-4_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Build System Developer | Task: Create main.less entry point importing all LESS files in correct dependency order following requirements REQ-1 and REQ-4, configure build-css.sh to compile main.less to main.css | Restrictions: Import order matters (variables → base → components → pages), must not create circular imports, ensure proper compilation | _Leverage: Check Mango's LESS imports for patterns_ | Success: main.less imports all files correctly, compiles to single main.css file, no compilation errors, correct cascading order | Instructions: Mark in-progress, verify import order, log with compilation details, mark complete._

- [x] 2.5. Update base.html to load compiled CSS
  - Files: `templates/base.html` (modify line 7-9)
  - Replace inline <style> with <link> to /static/dist/css/main.css
  - Remove inline CSS block (lines 9-318)
  - Purpose: Transition from inline CSS to external stylesheet
  - _Leverage: Keep existing functionality while swapping CSS source_
  - _Requirements: REQ-1, REQ-9_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Template Developer with Askama expertise | Task: Modify templates/base.html to load compiled CSS from /static/dist/css/main.css following requirements REQ-1 and REQ-9, remove inline <style> block (383 lines), test all pages render identically | Restrictions: Must not change any HTML structure, preserve {% block styles %}, test before committing, use Playwright for visual regression | _Leverage: Current base.html structure_ | Success: base.html loads external CSS successfully, all inline styles removed (0 lines), all pages render identically, no FOUC (Flash of Unstyled Content) | Instructions: Mark in-progress, take screenshots before/after, log with visual parity confirmation, mark complete._

### Phase 3: Extract and Consolidate Dark Theme

- [x] 3.1. Create dark theme LESS file
  - Files: `static/src/css/_dark-theme.less`
  - Extract ALL dark mode styles from base.html (lines 110-317)
  - Consolidate dark styles from library.css, book.css, home.css
  - Purpose: Single source of truth for all dark mode styling
  - _Leverage: Existing dark mode styles scattered across 5+ files_
  - _Requirements: REQ-5_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Theme Systems Developer | Task: Extract and consolidate ALL dark mode styles from templates/base.html (lines 110-317), static/css/library.css, static/css/book.css, static/css/home.css into single static/src/css/_dark-theme.less file following requirement REQ-5, use LESS variables for dark colors | Restrictions: MUST include every dark mode style (modals, forms, tables, alerts, etc.), use `body.uk-light` selector wrapper, organize by component | _Leverage: Current dark theme implementation across multiple files_ | Success: ALL dark mode styles consolidated into one file, no dark styles remain in other files, theme toggle works identically, all UI elements dark themed correctly | Instructions: Mark in-progress, grep for "uk-light" to find all dark styles, log with consolidation summary, mark complete._

- [ ] 3.2. Remove dark theme styles from individual CSS files
  - Files: `static/css/library.css`, `static/css/book.css`, `static/css/home.css` (delete dark sections)
  - Delete body.uk-light rules from page-specific CSS files
  - Verify dark theme still works via consolidated file
  - Purpose: Eliminate duplication and consolidate theme management
  - _Leverage: New _dark-theme.less as single source_
  - _Requirements: REQ-5_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CSS Refactoring Specialist | Task: Remove all `body.uk-light` dark mode styles from static/css/library.css, book.css, home.css following requirement REQ-5, verify dark theme continues working from consolidated _dark-theme.less | Restrictions: Must remove ALL dark theme styles, test each page in dark mode before/after, do not break light mode | _Leverage: Consolidated dark-theme.less as reference_ | Success: All body.uk-light rules removed from page CSS files, dark mode works identically, theme toggles correctly on all pages, no style regressions | Instructions: Mark in-progress, test dark mode thoroughly on each page, log with files modified, mark complete._

- [ ] 3.3. Test dark mode across all pages
  - Test: Navigate library, book, home, reader, admin pages
  - Toggle theme on each page, verify persistence
  - Check modals, forms, tables in dark mode
  - Purpose: Ensure consolidated dark theme works universally
  - _Leverage: Playwright MCP for comprehensive testing_
  - _Requirements: REQ-5, REQ-9_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Engineer specializing in visual testing | Task: Perform comprehensive dark mode testing across all pages following requirements REQ-5 and REQ-9, use Playwright MCP to test theme toggle, verify localStorage persistence, check all UI elements (modals, forms, tables, alerts, navigation) in both themes | Restrictions: Test EVERY page (/library, /book/:id, /home, /reader/*, /admin, /tags), verify no FOUC on page load, check system theme detection | _Leverage: Use Playwright MCP tools for browser testing_ | Success: Dark mode works perfectly on all pages, theme persists across navigation, no visual regressions, modals/forms/tables all dark themed, system theme detection works | Instructions: Mark in-progress, screenshot each page in both themes, log with testing summary and screenshots, mark complete._

### Phase 4: Move Reader CSS/JS to Static Files

- [x] 4.1. Move reader.css to static/src/css/pages/
  - Files: `templates/reader.css` → `static/src/css/pages/_reader.less`
  - Convert reader.css to LESS, remove from templates
  - Import in main.less
  - Purpose: Treat reader styles like other page styles
  - _Leverage: Reader styles from templates/reader.css (184 lines)_
  - _Requirements: REQ-1, REQ-2_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CSS Migration Specialist | Task: Move templates/reader.css (184 lines) to static/src/css/pages/_reader.less following requirements REQ-1 and REQ-2, convert to LESS format, remove dark theme styles (move to _dark-theme.less if not already there), import in main.less | Restrictions: Must remove {% include "reader.css" %} from reader.html, preserve all reader-specific styles, test reader page thoroughly | _Leverage: Current reader.css content_ | Success: reader.css moved successfully, compiled into main.css, reader.html loads external CSS, reader page renders identically, keyboard navigation works | Instructions: Mark in-progress, test reader functionality thoroughly, log with file migration details, mark complete._

- [x] 4.2. Move reader.js to static/src/js/pages/
  - Files: `templates/reader.js` → `static/src/js/pages/reader.js`
  - Copy reader.js to static folder, remove from templates
  - Update reader.html to use <script src="/static/js/pages/reader.js">
  - Purpose: Serve reader JavaScript as static asset
  - _Leverage: Reader JavaScript from templates/reader.js (495 lines)_
  - _Requirements: REQ-2_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: JavaScript Migration Developer | Task: Move templates/reader.js (495 lines) to static/src/js/pages/reader.js following requirement REQ-2, update templates/reader.html to load from /static/js/pages/reader.js instead of inline include, preserve all template variables ({{ title_id }}, {{ entry_id }}, {{ page }}) | Restrictions: Must preserve Askama template variable interpolation, test all reader functions (page navigation, keyboard shortcuts, progress saving), do not break functionality | _Leverage: Current reader.js logic_ | Success: reader.js moved to static/src/js/pages/, reader.html loads external script, all template variables work correctly, keyboard navigation functional, progress saving works | Instructions: Mark in-progress, test reader extensively, log with JavaScript module details, mark complete._

- [x] 4.3. Update reader.html template to load external assets
  - Files: `templates/reader.html` (modify)
  - Replace {% include "reader.css" %} with proper {% block styles %}
  - Replace {% include "reader.js" %} with proper {% block scripts %}
  - Purpose: Complete transition from inline to external assets
  - _Leverage: Current reader.html structure_
  - _Requirements: REQ-1, REQ-2_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Template Refactoring Developer | Task: Update templates/reader.html to load external CSS and JS following requirements REQ-1 and REQ-2, remove {% include %} directives, use {% block styles %} and {% block scripts %} properly to load from static/ | Restrictions: Must maintain base.html inheritance, preserve template variable access in JS, test reader functionality completely | _Leverage: reader.html current template structure_ | Success: reader.html has zero inline includes, loads CSS from main.css, loads JS from static/js/pages/reader.js, all reader functionality works identically | Instructions: Mark in-progress, test reader page end-to-end, log with template changes, mark complete._

### Phase 5: Create Template Components

- [x] 5.1. Create navigation component template
  - Files: `templates/components/nav.html`
  - Extract navigation HTML from base.html into reusable macro
  - Support parameters: active_page, is_admin, username
  - Purpose: Single source of truth for navigation markup
  - _Leverage: Current navigation HTML from base.html (lines 322-371)_
  - _Requirements: REQ-3_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Template Component Developer with Askama expertise | Task: Create reusable navigation component in templates/components/nav.html following requirement REQ-3, extract both desktop and mobile navigation from base.html (lines 322-371), create Askama macro accepting parameters (active_page, is_admin) | Restrictions: Must handle both desktop and mobile navigation in ONE component, use Askama macro syntax, maintain UIKit classes and attributes | _Leverage: base.html navigation structure_ | Success: nav.html component created with macro, accepts parameters correctly, single source for desktop AND mobile nav, eliminates duplication | Instructions: Mark in-progress, test navigation on all pages, log with component creation details, mark complete._

- [x] 5.2. Update base.html to use navigation component
  - Files: `templates/base.html` (modify lines 322-371)
  - Replace navigation HTML with {% from "components/nav.html" import nav %}
  - Call nav macro with appropriate parameters
  - Purpose: Use component instead of hardcoded navigation
  - _Leverage: New nav.html component from task 5.1_
  - _Requirements: REQ-3_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Template Integration Developer | Task: Update templates/base.html to use navigation component following requirement REQ-3, replace hardcoded navigation (lines 322-371) with {% from "components/nav.html" import nav %} and macro call, pass active_page and is_admin parameters | Restrictions: Must preserve all navigation functionality, hamburger menu must work, active page highlighting must work, test all pages | _Leverage: nav component from task 5.1_ | Success: base.html uses nav component successfully, navigation works on all pages, mobile hamburger functional, active highlighting correct, code duplication eliminated | Instructions: Mark in-progress, test navigation across pages, log with integration results, mark complete._

- [x] 5.3. Create modal base component template
  - Files: `templates/components/modal-base.html`
  - Create reusable modal structure with {% block %} for content
  - Support parameters: modal_id, title
  - Purpose: Standardize modal structure across application
  - _Leverage: Existing modals in book.html, home.html, admin.html_
  - _Requirements: REQ-3_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: UI Component Developer | Task: Create reusable modal base component in templates/components/modal-base.html following requirement REQ-3, analyze existing modals in book.html, home.html, admin.html to extract common structure, create macro with blocks for header, body, footer content | Restrictions: Must use UIKit modal structure, support both Alpine.js and vanilla modals, allow content customization via blocks | _Leverage: Existing modal patterns in current templates_ | Success: modal-base.html created with flexible structure, supports content blocks, works with UIKit, compatible with Alpine.js reactive state | Instructions: Mark in-progress, analyze existing modals, log with component structure, mark complete._

### Phase 6: Organize JavaScript Modules

- [x] 6.1. Create core JavaScript module
  - Files: `static/src/js/core.js`
  - Extract theme management from theme.js
  - Add localStorage utilities
  - Purpose: Shared JavaScript utilities loaded on all pages
  - _Leverage: Current static/js/theme.js (87 lines)_
  - _Requirements: REQ-2, REQ-5, REQ-7_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: JavaScript Module Developer | Task: Create static/src/js/core.js consolidating theme management from static/js/theme.js following requirements REQ-2, REQ-5, and REQ-7, add localStorage helper functions, create initialization logic | Restrictions: Must prevent FOUC (Flash of Unstyled Content), apply theme before DOM renders, support system theme detection, provide clean API for theme management | _Leverage: theme.js current implementation_ | Success: core.js created with theme management and utilities, theme applies before render (no FOUC), system theme detection works, provides toggleTheme(), getTheme(), savePreference() functions | Instructions: Mark in-progress, test theme functionality, log with module structure and exports, mark complete._

- [x] 6.2. Refactor library.js to use vanilla JavaScript for search
  - Files: `static/js/library.js` (modify)
  - Replace Alpine.js search with simple vanilla JavaScript
  - Keep Alpine.js for sort dropdown only
  - Purpose: Simplify library page JavaScript (Alpine.js overuse)
  - _Leverage: Current library.js Alpine implementation, original Mango's search.js pattern_
  - _Requirements: REQ-7_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: JavaScript Optimization Developer | Task: Refactor static/js/library.js following requirement REQ-7, replace Alpine.js search filtering with simple vanilla JavaScript (5-10 lines), keep Alpine only for reactive sort dropdown, import core.js utilities | Restrictions: Must maintain exact search functionality, case-insensitive matching, instant filtering, test search thoroughly | _Leverage: Original Mango's search.js pattern (simple jQuery), current Alpine implementation for reference_ | Success: Library search works with vanilla JS, code reduced from ~145 lines to ~100 lines, Alpine.js only used for sort dropdown, search performance maintained or improved | Instructions: Mark in-progress, test search extensively, log with code reduction metrics, mark complete._

- [x] 6.3. Refactor book.js to optimize Alpine.js usage
  - Files: `static/js/book.js` (modify)
  - Keep Alpine.js for entry modal (reactive state needed)
  - Simplify tag management code
  - Purpose: Optimize Alpine.js usage in book page
  - _Leverage: Current book.js Alpine implementation (223 lines)_
  - _Requirements: REQ-7_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: JavaScript Refactoring Specialist | Task: Optimize static/js/book.js following requirement REQ-7, keep Alpine.js for entry modal (reactive state management appropriate here), simplify tag autocomplete logic, remove unnecessary reactivity | Restrictions: Must preserve all functionality (entry modal, progress management, tag autocomplete), reduce code complexity where possible, maintain user experience | _Leverage: Current book.js structure_ | Success: book.js optimized, Alpine.js used appropriately for reactive components, tag management simplified, all functionality works identically, code is cleaner | Instructions: Mark in-progress, test book page features, log with optimization details, mark complete._

- [x] 6.4. Update base.html to load core.js
  - Files: `templates/base.html` (modify script loading)
  - Load core.js before page-specific scripts
  - Ensure theme applies before render
  - Purpose: Provide core utilities to all pages
  - _Leverage: New core.js module from task 6.1_
  - _Requirements: REQ-2, REQ-5_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Template Script Integration Developer | Task: Update templates/base.html to load static/js/core.js following requirements REQ-2 and REQ-5, place script load in <head> with defer attribute to prevent FOUC, ensure core.js loads before page-specific scripts | Restrictions: Must prevent FOUC, use defer or async appropriately, maintain script execution order, test theme application | _Leverage: core.js from task 6.1_ | Success: core.js loads on all pages, theme applies before render (no FOUC), core utilities available to page scripts, script loading order correct | Instructions: Mark in-progress, test on multiple pages, log with script loading configuration, mark complete._

### Phase 7: Page-Specific CSS Migration

- [x] 7.1. Move library.css to LESS
  - Files: `static/css/library.css` → `static/src/css/pages/_library.less`
  - Convert library.css to LESS format
  - Remove dark theme styles (already in _dark-theme.less)
  - Purpose: Consolidate all CSS into compiled bundle
  - _Leverage: Current library.css (243 lines)_
  - _Requirements: REQ-1, REQ-5_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CSS Migration Developer | Task: Convert static/css/library.css to static/src/css/pages/_library.less following requirements REQ-1 and REQ-5, remove dark theme styles (should be in _dark-theme.less), use LESS variables and nesting, import in main.less | Restrictions: Remove ALL body.uk-light rules, use variables for colors, test library page in both themes, verify identical rendering | _Leverage: library.css current styles_ | Success: library.css converted to _library.less, dark styles removed, compiled into main.css, library page renders identically in both themes | Instructions: Mark in-progress, test library page thoroughly, log with migration details, mark complete._

- [x] 7.2. Move book.css to LESS
  - Files: `static/css/book.css` → `static/src/css/pages/_book.less`
  - Convert book.css to LESS format
  - Remove dark theme styles (already in _dark-theme.less)
  - Purpose: Consolidate all CSS into compiled bundle
  - _Leverage: Current book.css (333 lines)_
  - _Requirements: REQ-1, REQ-5_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CSS Migration Developer | Task: Convert static/css/book.css to static/src/css/pages/_book.less following requirements REQ-1 and REQ-5, remove dark theme styles (should be in _dark-theme.less), use LESS variables and nesting, import in main.less | Restrictions: Remove ALL body.uk-light rules, use variables for colors, test book page in both themes, verify identical rendering including modals and tags | _Leverage: book.css current styles_ | Success: book.css converted to _book.less, dark styles removed, compiled into main.css, book page renders identically, entry modal and tags work correctly | Instructions: Mark in-progress, test book page and modals, log with migration details, mark complete._

- [x] 7.3. Move home.css to LESS
  - Files: `static/css/home.css` → `static/src/css/pages/_home.less`
  - Convert home.css to LESS format
  - Remove dark theme styles (already in _dark-theme.less)
  - Purpose: Consolidate all CSS into compiled bundle
  - _Leverage: Current home.css (232 lines)_
  - _Requirements: REQ-1, REQ-5_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: CSS Migration Developer | Task: Convert static/css/home.css to static/src/css/pages/_home.less following requirements REQ-1 and REQ-5, remove dark theme styles (should be in _dark-theme.less), use LESS variables and nesting, import in main.less | Restrictions: Remove ALL body.uk-light rules, use variables for colors, test home page in both themes, verify continue reading, start reading, recently added sections | _Leverage: home.css current styles_ | Success: home.css converted to _home.less, dark styles removed, compiled into main.css, home page renders identically, all sections (continue/start/recent) work correctly | Instructions: Mark in-progress, test home page sections, log with migration details, mark complete._

- [x] 7.4. Update templates to remove individual CSS file loads
  - Files: `templates/library.html`, `templates/book.html`, `templates/home.html` (modify {% block styles %})
  - Remove <link> tags for individual CSS files
  - All styles now loaded from main.css in base.html
  - Purpose: Simplify template CSS loading
  - _Leverage: Compiled main.css includes all page styles_
  - _Requirements: REQ-1_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Template Cleanup Developer | Task: Update templates/library.html, book.html, home.html following requirement REQ-1, remove {% block styles %} overrides that load individual CSS files, verify all styles load from main.css in base.html, test visual parity | Restrictions: Do not remove {% block styles %} definition, just remove content, verify pages render identically, check both themes | _Leverage: main.css compiled from LESS sources_ | Success: Individual CSS file loads removed, all pages use main.css exclusively, visual appearance identical, both themes work correctly | Instructions: Mark in-progress, test each page, log with template simplification, mark complete._

### Phase 8: Testing and Finalization

- [x] 8.1. Comprehensive visual regression testing
  - Test: Screenshot all pages in light and dark mode
  - Compare before/after refactoring screenshots
  - Verify pixel-perfect parity
  - Purpose: Ensure refactoring didn't break visual appearance
  - _Leverage: Playwright MCP for screenshot testing_
  - _Requirements: All requirements_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Visual Testing Specialist | Task: Perform comprehensive visual regression testing covering all requirements, use Playwright MCP to screenshot all pages (/library, /book/:id, /home, /reader/*, /admin, /tags, /change-password) in both light and dark themes, compare with pre-refactoring screenshots | Restrictions: Must test EVERY page, both themes, multiple screen sizes (desktop, tablet, mobile), identify any visual differences | _Leverage: Playwright MCP browser_take_screenshot tool_ | Success: All pages render identically to pre-refactoring, zero visual regressions, both themes work perfectly, mobile responsive layouts correct | Instructions: Mark in-progress, screenshot systematically, log with visual testing summary and any issues found, mark complete._

- [x] 8.2. Functional testing of all interactive features
  - Test: Navigation, theme toggle, search, modals, forms
  - Verify keyboard shortcuts, progress saving, tag management
  - Test on multiple browsers
  - Purpose: Ensure refactoring didn't break functionality
  - _Leverage: Playwright MCP for automated testing_
  - _Requirements: All requirements_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: QA Functional Testing Engineer | Task: Perform comprehensive functional testing covering all requirements, test navigation (hamburger menu, links), theme toggle and persistence, library search and sort, reader keyboard shortcuts, book entry modals, tag autocomplete, admin functions | Restrictions: Test ALL interactive features, verify localStorage persistence, test across pages, check edge cases | _Leverage: Playwright MCP browser interaction tools_ | Success: All functionality works identically to pre-refactoring, no broken features, interactions smooth, data persists correctly | Instructions: Mark in-progress, test systematically, log with functional testing summary, mark complete._

- [x] 8.3. Performance testing and optimization
  - Test: Measure page load times, CSS bundle size, JavaScript size
  - Verify First Contentful Paint <1s, Time to Interactive <2s
  - Check minification and compression
  - Purpose: Ensure performance meets or exceeds requirements
  - _Leverage: Browser DevTools, Lighthouse_
  - _Requirements: REQ-9_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Performance Testing Specialist | Task: Measure performance metrics following requirement REQ-9, use browser DevTools and Lighthouse to measure First Contentful Paint, Time to Interactive, CSS bundle size (<50KB gzipped), JavaScript size (<100KB total), verify cache headers correct | Restrictions: Test on 3G connection simulation, measure multiple pages, identify any performance regressions | _Leverage: Chrome DevTools Performance panel, Lighthouse_ | Success: Performance meets targets (FCP <1s, TTI <2s on 3G), CSS <50KB gzipped, JS <100KB total, cache headers correct, performance improved or maintained | Instructions: Mark in-progress, measure systematically, log with performance metrics, mark complete._

- [-] 8.4. Update documentation (README.md)
  - Files: `README.md` (update), create `FRONTEND.md`
  - Document build system setup and usage
  - Explain directory structure and file organization
  - Purpose: Help developers understand new frontend architecture
  - _Leverage: Current README.md structure_
  - _Requirements: REQ-10_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Technical Documentation Writer | Task: Update README.md and create FRONTEND.md documentation following requirement REQ-10, document build system setup (npm install -g less), development workflow (watch-css.sh), production build (build-css.sh), directory structure explanation, file organization standards | Restrictions: Keep documentation concise and practical, include prerequisites, setup steps, development workflow, troubleshooting common issues | _Leverage: Current README.md format_ | Success: Documentation complete and clear, setup instructions work for new developers, directory structure explained, development workflow documented | Instructions: Mark in-progress, write comprehensive docs, log with documentation details, mark complete._

- [ ] 8.5. Final cleanup and code review
  - Task: Remove old CSS files, delete unused static files
  - Review all code for consistency and quality
  - Ensure .gitignore is correct
  - Purpose: Clean up legacy files and finalize refactoring
  - _Leverage: Git status to find unused files_
  - _Requirements: REQ-10_
  - _Prompt: Implement the task for spec frontend-refactor, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Senior Code Reviewer | Task: Perform final cleanup following requirement REQ-10, delete old static/css/library.css, book.css, home.css, remove templates/reader.css and reader.js, verify .gitignore excludes static/dist/, review all code for consistency (kebab-case naming, LESS organization, JS module structure) | Restrictions: Only delete files that are now compiled into bundles, verify no broken references, test after deletion, commit with clear message | _Leverage: Git to track file removals_ | Success: All legacy files removed, no broken references, .gitignore correct, code follows consistent standards, project structure clean | Instructions: Mark in-progress, clean systematically, log with cleanup summary and files removed, mark complete._

## Summary

**Total Tasks:** 33 tasks across 8 phases
**Estimated Time:** 2-3 days of focused work
**Order:** Must be completed sequentially (each phase depends on previous)

**Success Metrics:**
- Zero inline CSS/JS in templates
- All styles compiled from LESS sources
- CSS bundle <50KB gzipped
- JavaScript organized into logical modules
- Template components eliminate duplication
- Visual and functional parity with pre-refactoring
- Performance meets or exceeds targets
- Code is maintainable and well-documented
