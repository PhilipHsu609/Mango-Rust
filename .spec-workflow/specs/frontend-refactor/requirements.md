# Requirements Document: Frontend Refactoring

## Introduction

The Mango-Rust frontend currently suffers from poor code organization, with 383 lines of inline CSS in base.html, scattered JavaScript files, and inconsistent use of modern frameworks. This refactoring aims to create a clean, maintainable frontend architecture that **exceeds** the original Mango's quality while maintaining all existing functionality.

The refactoring will focus on:
- Extracting inline styles to proper stylesheets
- Organizing CSS with a build system (LESS compilation)
- Consolidating JavaScript into logical modules
- Improving Alpine.js usage patterns
- Creating reusable template components
- Enhancing mobile responsiveness
- Maintaining dark mode functionality

## Alignment with Product Vision

This refactoring directly supports the core principle: **"Be better than the original Mango, not worse."** By fixing architectural issues identified in the Linus analysis, we will:

1. **Improve maintainability** - Easier for contributors to understand and modify
2. **Enhance performance** - Proper asset compilation and caching
3. **Enable future features** - Clean architecture supports rapid development
4. **Professional quality** - Code organization matches enterprise standards

## Requirements

### REQ-1: CSS Architecture Cleanup

**User Story:** As a developer, I want all CSS properly organized in separate files, so that I can maintain and extend styles without editing massive inline style blocks.

#### Acceptance Criteria

1. WHEN viewing base.html THEN it SHALL contain ZERO inline style blocks
2. WHEN dark mode is toggled THEN all theme styles SHALL load from /static/css/dark-theme.css
3. WHEN a page loads THEN CSS SHALL be served from compiled static files with proper caching headers
4. WHEN modifying styles THEN developers SHALL edit LESS source files in static/src/css/
5. IF a component has specific styles THEN those styles SHALL be in a separate file named after the component

**Metrics:**
- base.html: 0 lines of inline CSS (currently 383 lines)
- All templates: 0 lines of inline CSS
- CSS files organized in logical hierarchy

### REQ-2: JavaScript Module Organization

**User Story:** As a developer, I want JavaScript organized into logical modules with clear responsibilities, so that I can understand and modify behavior without hunting through multiple files.

#### Acceptance Criteria

1. WHEN viewing reader.html THEN JavaScript SHALL be loaded from /static/js/reader.js (not inline)
2. WHEN a page needs JavaScript THEN it SHALL load only the modules it requires
3. WHEN Alpine.js is used THEN it SHALL be for reactive state management only, not simple DOM manipulation
4. WHEN vanilla JavaScript suffices THEN Alpine.js SHALL NOT be used
5. IF JavaScript is shared across pages THEN it SHALL be in /static/js/core.js

**Metrics:**
- Zero inline <script> blocks with Askama includes
- Clear separation: core.js, pages/*.js pattern
- Alpine.js usage reduced by >50% for non-reactive cases

### REQ-3: Template Component System

**User Story:** As a developer, I want reusable template components, so that I don't duplicate navigation, modals, or other UI elements across multiple files.

#### Acceptance Criteria

1. WHEN rendering navigation THEN both mobile and desktop SHALL use the same component source
2. WHEN adding a navigation item THEN it SHALL only need to be added in ONE location
3. WHEN a modal is used THEN it SHALL extend a base modal component
4. WHEN components are included THEN they SHALL use {% include "components/name.html" %}
5. IF a component needs data THEN it SHALL be passed via template parameters, not globals

**Metrics:**
- Navigation code duplicated: 0 times (currently 2x)
- Number of reusable components: >5
- Lines of template code reduced by >30%

### REQ-4: Asset Build System

**User Story:** As a developer, I want a proper asset build pipeline, so that CSS/JS is compiled, minified, and versioned automatically.

#### Acceptance Criteria

1. WHEN LESS files are modified THEN they SHALL be compiled to CSS automatically
2. WHEN JavaScript is built THEN it SHALL be minified for production
3. WHEN assets are served THEN they SHALL have cache-busting versioning
4. WHEN running in development THEN changes SHALL hot-reload without manual restart
5. IF a build fails THEN clear error messages SHALL be displayed

**Metrics:**
- Build time: <5 seconds for full rebuild
- CSS file size reduced by >40% (minification + deduplication)
- Zero manual compilation steps required

### REQ-5: Dark Mode Consolidation

**User Story:** As a user, I want dark mode to work consistently across all pages, so that my theme preference persists and doesn't flash on page load.

#### Acceptance Criteria

1. WHEN dark mode is enabled THEN ALL dark theme CSS SHALL be in /static/css/dark-theme.css
2. WHEN a page loads THEN theme preference SHALL be applied before render (no flash)
3. WHEN toggling theme THEN the change SHALL be instant across all elements
4. WHEN viewing different pages THEN theme SHALL persist via localStorage
5. IF system theme changes THEN Mango SHALL detect and apply it automatically

**Metrics:**
- Dark mode CSS: 1 file (currently scattered across 5+ files)
- Theme application time: <16ms (1 frame)
- Zero FOUC (Flash of Unstyled Content)

### REQ-6: Mobile Responsiveness Enhancement

**User Story:** As a mobile user, I want a responsive UI that works perfectly on small screens, so that I can read manga comfortably on my phone.

#### Acceptance Criteria

1. WHEN viewing on mobile THEN hamburger menu SHALL be properly aligned and styled
2. WHEN navigation is opened THEN the offcanvas SHALL slide smoothly with proper animation
3. WHEN cards are displayed THEN grid layout SHALL be optimal for screen size
4. WHEN reading on mobile THEN no horizontal scrolling SHALL occur
5. IF screen is <960px THEN mobile-optimized layout SHALL be used

**Metrics:**
- Mobile navigation animation: <300ms
- Touch target size: ≥44x44px for all interactive elements
- Zero horizontal scroll on any page

### REQ-7: Alpine.js Best Practices

**User Story:** As a developer, I want Alpine.js used only where it adds value, so that we don't over-engineer simple interactions.

#### Acceptance Criteria

1. WHEN implementing search filtering THEN Alpine.js SHALL be used for reactive state
2. WHEN implementing tag autocomplete THEN Alpine.js SHALL manage the suggestion state
3. WHEN implementing simple click handlers THEN vanilla JavaScript SHALL be used
4. WHEN Alpine components are created THEN they SHALL follow Alpine v3 composition patterns
5. IF Alpine is not needed THEN vanilla JavaScript SHALL be used instead

**Metrics:**
- Alpine.js components: Only for reactive/stateful UI (<10 total)
- Vanilla JS for simple DOM: >80% of basic interactions
- Alpine bundle size: Not loaded on pages that don't need it

### REQ-8: UIKit Framework Consistency

**User Story:** As a developer, I want consistent UIKit usage patterns, so that the UI behaves predictably and uses framework features properly.

#### Acceptance Criteria

1. WHEN using UIKit components THEN they SHALL use proper UIKit classes and attributes
2. WHEN modals are needed THEN UIKit modals SHALL be used (not custom implementations)
3. WHEN grids are needed THEN UIKit grid classes SHALL be used consistently
4. WHEN forms are rendered THEN UIKit form styles SHALL be applied
5. IF custom CSS is needed THEN it SHALL extend UIKit, not override it

**Metrics:**
- UIKit version: Consistent across all pages (3.5.9 or upgrade to 3.19+)
- Custom CSS overriding UIKit: <5% of total styles
- UIKit component usage: >90% where applicable

### REQ-9: Performance Optimization

**User Story:** As a user, I want pages to load quickly, so that I can start reading manga without delays.

#### Acceptance Criteria

1. WHEN a page loads THEN CSS SHALL be loaded in <100ms
2. WHEN JavaScript is loaded THEN only required modules SHALL be fetched
3. WHEN images are loaded THEN they SHALL have proper dimensions to prevent layout shift
4. WHEN assets are cached THEN browser SHALL reuse them for 1 year
5. IF assets are updated THEN cache SHALL be busted via versioning

**Metrics:**
- First Contentful Paint: <1s on 3G connection
- Time to Interactive: <2s on 3G connection
- CSS file size: <50KB compressed
- JavaScript file size: <100KB compressed total

### REQ-10: Code Organization Standards

**User Story:** As a developer, I want clear file organization standards, so that I know exactly where to find and place code.

#### Acceptance Criteria

1. WHEN writing CSS THEN source files SHALL be in static/src/css/ organized by purpose
2. WHEN writing JavaScript THEN files SHALL be in static/src/js/ organized by scope
3. WHEN creating templates THEN reusable components SHALL be in templates/components/
4. WHEN naming files THEN kebab-case SHALL be used consistently
5. IF a file exceeds 300 lines THEN it SHALL be refactored into smaller modules

**Metrics:**
- Maximum file size: <300 lines (except compiled output)
- File naming consistency: 100% kebab-case
- Component organization: All components in components/ directory

## Non-Functional Requirements

### Code Architecture and Modularity

- **Single Responsibility Principle**: Each CSS/JS file has one clear purpose
  - base.css: Core layout and typography
  - dark-theme.css: ALL dark mode styles
  - components/*.css: Individual component styles
  - core.js: Shared utilities and initialization
  - pages/*.js: Page-specific behavior

- **Modular Design**: Components are isolated and reusable
  - Template components in templates/components/
  - CSS components in static/src/css/components/
  - JS modules in static/src/js/modules/

- **Dependency Management**: Minimize interdependencies
  - Core JS has no page dependencies
  - Page JS imports only what it needs
  - CSS uses LESS imports for shared variables

- **Clear Interfaces**: Define clean contracts
  - Template components accept explicit parameters
  - JS modules export clear public APIs
  - CSS classes follow BEM or similar convention

### Performance

- **First Load**: <2s on 3G (library page)
- **Subsequent Loads**: <500ms (cached assets)
- **CSS Bundle**: <50KB gzipped
- **JavaScript Bundle**: <100KB gzipped total
- **Build Time**: <5s for full rebuild

### Security

- **Content Security Policy**: All inline styles/scripts eliminated (allows strict CSP)
- **XSS Prevention**: No dynamic HTML injection in JS
- **Asset Integrity**: Subresource Integrity (SRI) for CDN assets
- **HTTPS Only**: All assets served over HTTPS

### Reliability

- **Backward Compatibility**: All existing functionality must work identically
- **Browser Support**: Modern browsers (last 2 versions of Chrome/Firefox/Safari/Edge)
- **Graceful Degradation**: Core functionality works without JavaScript
- **Error Handling**: Build failures provide clear error messages

### Usability

- **No Visual Changes**: UI should look identical to current implementation
- **No Behavior Changes**: All interactions work exactly as before
- **Dark Mode Parity**: Theme switching works identically
- **Mobile Parity**: Mobile experience is same or better

### Maintainability

- **Documentation**: README.md explains build system and file organization
- **Code Comments**: Complex logic is commented
- **Naming Conventions**: Consistent kebab-case for files, camelCase for JS, kebab-case for CSS classes
- **Git History**: Each refactoring step is a separate, well-documented commit

## Success Criteria

The frontend refactoring is considered successful when:

1. ✅ Zero inline CSS/JS in any template file
2. ✅ All styles organized in static/src/css/ and compiled to static/dist/css/
3. ✅ All JavaScript organized in static/src/js/ and compiled to static/dist/js/
4. ✅ Build system compiles LESS to CSS automatically
5. ✅ Template components eliminate code duplication
6. ✅ Dark mode works identically to current implementation
7. ✅ All pages render identically to current implementation
8. ✅ Performance metrics meet or exceed requirements
9. ✅ Code is easier to understand and modify than current implementation
10. ✅ Linus would approve (or at least complain less)

## Out of Scope

- UI/UX redesign (visual appearance stays the same)
- Backend changes (Rust code unchanged)
- New features (this is purely refactoring)
- Framework changes (keep UIKit 3.5.9, Alpine.js 3.13.3)
- Database schema changes
- API endpoint modifications
