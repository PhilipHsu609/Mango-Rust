# Requirements Document

## Introduction

The current theme implementation in Mango-Rust has a semantic naming inconsistency that creates confusion for developers. The dark theme is activated by adding a `uk-light` CSS class to the body element, which is backwards and counter-intuitive. While this follows UIkit's convention where `uk-light` means "light text on dark background", it conflicts with modern theme naming patterns where class names should represent the theme itself, not the text treatment.

This refactoring will create a semantically correct, intuitive theme system using `uk-dark` for dark theme and `uk-light` for light theme, improving code maintainability and developer experience.

**Current State Analysis:**
- Dark theme: JavaScript sets `body.uk-light` class, CSS targets `body.uk-light` selector
- Light theme: No explicit class (default/baseline styles), has `_uikit-light.less` for UIkit overrides
- Problem: `uk-light` activates dark mode, creating cognitive dissonance

## Alignment with Product Vision

This refactoring supports Mango's commitment to:
- **Code Quality**: Creating maintainable, understandable code that follows semantic naming conventions
- **Developer Experience**: Making the codebase intuitive for contributors and future maintainers
- **Modern Standards**: Aligning with contemporary web development practices for theme management

## Requirements

### Requirement 1: Semantic Dark Theme Implementation

**User Story:** As a developer, I want the dark theme activated by a `uk-dark` class, so that the code is immediately understandable without confusion

#### Acceptance Criteria

1. WHEN dark theme is active THEN `body` element SHALL have `uk-dark` class applied
2. WHEN dark theme CSS is defined THEN styles SHALL use `body.uk-dark` selector
3. WHEN dark theme is toggled THEN JavaScript SHALL add/remove `uk-dark` class from body
4. WHEN dark theme file is examined THEN it SHALL be named `_dark-theme.less` and use `body.uk-dark` selector

### Requirement 2: Semantic Light Theme Implementation

**User Story:** As a developer, I want the light theme explicitly represented with a `uk-light` class, so that both themes have equal, clear representation

#### Acceptance Criteria

1. WHEN light theme is active THEN `body` element SHALL have `uk-light` class applied
2. WHEN light theme CSS is defined THEN styles SHALL use `body.uk-light` selector
3. WHEN light theme is toggled THEN JavaScript SHALL add/remove `uk-light` class from body
4. WHEN light theme file is examined THEN it SHALL be named `_light-theme.less` and use `body.uk-light` selector
5. WHEN neither theme class is present THEN fallback styles SHALL provide readable defaults

### Requirement 3: Clean Theme Switching

**User Story:** As a user, I want theme switching to work seamlessly without visual glitches, so that my reading experience is uninterrupted

#### Acceptance Criteria

1. WHEN theme is toggled THEN only one theme class SHALL be active at a time
2. WHEN switching themes THEN no Flash of Unstyled Content (FOUC) SHALL occur
3. WHEN page loads THEN saved theme preference SHALL be applied before first render
4. WHEN system theme changes THEN application SHALL respect system preference if no manual override exists

### Requirement 4: Backwards Compatibility During Transition

**User Story:** As a user, I want my saved theme preference preserved, so that my experience is not disrupted by the refactoring

#### Acceptance Criteria

1. WHEN existing theme preference exists in localStorage THEN it SHALL continue to work correctly
2. WHEN theme preference is 'dark' THEN new code SHALL apply `uk-dark` class
3. WHEN theme preference is 'light' THEN new code SHALL apply `uk-light` class
4. WHEN system theme is detected THEN it SHALL map correctly to new class names

## Non-Functional Requirements

### Code Architecture and Modularity

- **Single Responsibility Principle**:
  - `_dark-theme.less`: Contains ONLY dark theme overrides using `body.uk-dark` selector
  - `_light-theme.less`: Contains ONLY light theme overrides using `body.uk-light` selector
  - `_variables.less`: Contains theme-agnostic color tokens and design variables
  - `core.js`: Theme management logic isolated in dedicated functions

- **Modular Design**:
  - Theme files can be modified independently without affecting each other
  - Theme logic in JavaScript is isolated from other DOM manipulation
  - CSS cascade order ensures proper theme application (light base → dark overrides)

- **Clear Interfaces**:
  - Consistent theme class naming: `uk-dark` for dark, `uk-light` for light
  - JavaScript API: `getTheme()`, `applyTheme(theme)`, `toggleTheme()`
  - localStorage contract: Store 'dark' or 'light' string values

### Performance

- Theme application SHALL complete before first contentful paint (prevent FOUC)
- Theme switching SHALL complete within 16ms (single frame at 60fps)
- No layout thrashing during theme application
- Minimal CSS specificity to ensure fast selector matching

### Security

- No user-supplied content SHALL influence theme class names
- Theme preference SHALL be validated before localStorage write
- XSS protection through strict string literals for class names

### Reliability

- Theme state SHALL persist across browser sessions via localStorage
- System theme detection SHALL gracefully degrade if matchMedia unavailable
- Theme SHALL default to light mode if localStorage corrupted or unavailable

### Usability

- Theme naming SHALL be immediately intuitive to developers (`uk-dark` = dark theme)
- Console errors or warnings SHALL clearly indicate theme-related issues
- Documentation comments SHALL explain the dual-class approach (light and dark)
- Code organization SHALL make theme files easy to locate and modify
