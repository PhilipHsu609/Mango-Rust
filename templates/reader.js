// Constants from backend
const TITLE_ID = '{{ title_id }}';
const ENTRY_ID = '{{ entry_id }}';
const CURRENT_PAGE = {{ current_page }};
const TOTAL_PAGES = {{ total_pages }};
const NEXT_ENTRY_URL = {% if let Some(url) = next_entry_url %}'{{ url }}'{% else %}null{% endif %};
const EXIT_URL = '{{ exit_url }}';

// State
let currentPage = CURRENT_PAGE;
let mode = 'paged';
let fit = 'height';
let margin = 20;
let enableAnimation = true;
let preloadCount = 2;
let enableRTL = false;
let loadedImages = new Set();
let preloadedImages = {};

// Elements
const loading = document.getElementById('loading');
const continuousContainer = document.getElementById('continuous-container');
const pagedContainer = document.getElementById('paged-container');
const pagedImage = document.getElementById('paged-image');
const modeSelect = document.getElementById('mode-select');
const fitSelect = document.getElementById('fit-select');
const marginRange = document.getElementById('margin-range');
const marginValue = document.getElementById('margin-value');
const preloadRange = document.getElementById('preload-range');
const preloadValue = document.getElementById('preload-value');
const pageSelect = document.getElementById('page-select');
const progressText = document.getElementById('progress-text');
const fitSection = document.getElementById('fit-section');
const marginSection = document.getElementById('margin-section');
const animationSection = document.getElementById('animation-section');
const preloadSection = document.getElementById('preload-section');
const rtlSection = document.getElementById('rtl-section');

// Load saved preferences
function loadPreferences() {
    const savedMode = localStorage.getItem('reader-mode');
    const savedFit = localStorage.getItem('reader-fit');
    const savedMargin = localStorage.getItem('reader-margin');
    const savedAnimation = localStorage.getItem('reader-animation');
    const savedPreload = localStorage.getItem('reader-preload');
    const savedRTL = localStorage.getItem('reader-rtl');

    if (savedMode) {
        mode = savedMode;
        modeSelect.value = mode;
    }
    if (savedFit) {
        fit = savedFit;
        fitSelect.value = fit;
    }
    if (savedMargin) {
        margin = parseInt(savedMargin);
        marginRange.value = margin;
        marginValue.textContent = margin;
    }
    if (savedAnimation !== null) {
        enableAnimation = savedAnimation === 'true';
        document.getElementById('enable-flip-animation').checked = enableAnimation;
    }
    if (savedPreload) {
        preloadCount = parseInt(savedPreload);
        preloadRange.value = preloadCount;
        preloadValue.textContent = preloadCount;
    }
    if (savedRTL !== null) {
        enableRTL = savedRTL === 'true';
        document.getElementById('enable-rtl').checked = enableRTL;
    }
}

// Save preferences
function savePreferences() {
    localStorage.setItem('reader-mode', mode);
    localStorage.setItem('reader-fit', fit);
    localStorage.setItem('reader-margin', margin);
    localStorage.setItem('reader-animation', enableAnimation);
    localStorage.setItem('reader-preload', preloadCount);
    localStorage.setItem('reader-rtl', enableRTL);
}

// Get image URL for a page
function getPageUrl(page) {
    return `/api/page/${TITLE_ID}/${ENTRY_ID}/${page}`;
}

// Update progress display
function updateProgress() {
    const percent = ((currentPage / TOTAL_PAGES) * 100).toFixed(1);
    progressText.textContent = `Progress: ${currentPage}/${TOTAL_PAGES} (${percent}%)`;
    pageSelect.value = currentPage;
}

// Save progress to server
async function saveProgress() {
    try {
        await fetch(`/api/progress/${TITLE_ID}/${ENTRY_ID}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ page: currentPage })
        });
    } catch (error) {
        console.error('Failed to save progress:', error);
    }
}

// Save progress using sendBeacon (for page unload)
function saveProgressBeacon() {
    const data = JSON.stringify({ page: currentPage });
    const blob = new Blob([data], { type: 'application/json' });
    navigator.sendBeacon(`/api/progress/${TITLE_ID}/${ENTRY_ID}`, blob);
}

// Preload images
function preloadImages() {
    if (mode !== 'paged' || preloadCount === 0) return;

    for (let i = 1; i <= preloadCount; i++) {
        const nextPage = currentPage + i;
        if (nextPage <= TOTAL_PAGES && !preloadedImages[nextPage]) {
            const img = new Image();
            img.src = getPageUrl(nextPage);
            preloadedImages[nextPage] = img;
        }

        const prevPage = currentPage - i;
        if (prevPage >= 1 && !preloadedImages[prevPage]) {
            const img = new Image();
            img.src = getPageUrl(prevPage);
            preloadedImages[prevPage] = img;
        }
    }
}

// Initialize continuous mode
function initContinuousMode() {
    loading.style.display = 'block';
    continuousContainer.innerHTML = '';
    continuousContainer.style.visibility = 'hidden'; // Hide container initially

    let loadedCount = 0;
    const targetPage = currentPage;

    for (let page = 1; page <= TOTAL_PAGES; page++) {
        const img = document.createElement('img');
        img.className = 'continuous-image';
        img.src = getPageUrl(page);
        img.alt = `Page ${page}`;
        img.style.marginTop = `${margin}px`;
        img.style.marginBottom = `${margin}px`;
        img.dataset.page = page;

        // Mark as spine if needed (detect narrow images)
        img.onload = function() {
            if (this.naturalWidth < 50) {
                this.classList.add('spine');
            }
            loadedCount++;

            // Once we've loaded the target page, scroll to it
            if (parseInt(this.dataset.page) === targetPage) {
                scrollToCurrentPage();
                // Show container and hide loading after scroll
                continuousContainer.style.visibility = 'visible';
                loading.style.display = 'none';
            }
        };

        img.onclick = function() {
            currentPage = parseInt(this.dataset.page);
            updateProgress();
            saveProgress();
        };

        continuousContainer.appendChild(img);
    }

    // Add next entry / exit button
    const button = document.createElement('button');
    button.id = 'next-entry-btn';
    button.className = 'uk-button uk-button-primary';
    if (NEXT_ENTRY_URL) {
        button.textContent = 'Next Entry';
        button.onclick = () => window.location.href = NEXT_ENTRY_URL;
    } else {
        button.textContent = 'Exit Reader';
        button.onclick = () => window.location.href = EXIT_URL;
    }
    continuousContainer.appendChild(button);

    // Fallback: if target image doesn't load, show container anyway after 1 second
    setTimeout(() => {
        if (continuousContainer.style.visibility === 'hidden') {
            scrollToCurrentPage();
            continuousContainer.style.visibility = 'visible';
            loading.style.display = 'none';
        }
    }, 1000);

    // Track visible page for progress - delay setup until after images load and scroll
    setTimeout(() => {
        setupScrollHandler();
    }, 500);
}

// Scroll to the current page in continuous mode
function scrollToCurrentPage() {
    const currentImg = continuousContainer.querySelector(`[data-page="${currentPage}"]`);
    if (currentImg && currentImg.complete) {
        currentImg.scrollIntoView({ behavior: 'instant', block: 'start' });
    }
}

// Scroll handler for continuous mode using scroll events
function setupScrollHandler() {
    let scrollTimeout;
    let isInitialLoad = true;

    function findCurrentPage() {
        const images = document.querySelectorAll('.continuous-image');
        let bestMatch = null;
        let bestVisiblePercent = 0;

        images.forEach(img => {
            const rect = img.getBoundingClientRect();
            const viewportHeight = window.innerHeight;

            // Calculate how much of the image is visible
            const visibleTop = Math.max(rect.top, 0);
            const visibleBottom = Math.min(rect.bottom, viewportHeight);
            const visibleHeight = Math.max(0, visibleBottom - visibleTop);
            const visiblePercent = (visibleHeight / viewportHeight) * 100;

            // The page that takes up the most viewport space is considered "current"
            if (visiblePercent > bestVisiblePercent) {
                bestVisiblePercent = visiblePercent;
                bestMatch = parseInt(img.dataset.page);
            }
        });

        if (bestMatch && bestMatch !== currentPage) {
            currentPage = bestMatch;
            updateProgress();
            saveProgress();
            // Update URL to reflect current page
            const newUrl = `/reader/${TITLE_ID}/${ENTRY_ID}/${currentPage}`;
            history.replaceState(null, '', newUrl);
            document.title = `${document.title.split(' - Page')[0]} - Page ${currentPage}`;
        }

        // Mark as initialized after first check (after initial scroll has settled)
        if (isInitialLoad) {
            isInitialLoad = false;
            hasInteracted = true; // Now that we're at the right page, mark as interacted
        }
    }

    // Don't check immediately - wait for initial scroll to complete
    // Check on scroll with debouncing
    window.addEventListener('scroll', () => {
        clearTimeout(scrollTimeout);
        scrollTimeout = setTimeout(findCurrentPage, 100);
    });

    // Also check after a delay to catch the position after initial scroll
    setTimeout(findCurrentPage, 1500);
}

// Initialize paged mode
function initPagedMode() {
    loadPage(currentPage);
    preloadImages();
}

// Load a page in paged mode
function loadPage(page) {
    if (page < 1 || page > TOTAL_PAGES) return;

    loading.style.display = 'block';
    pagedImage.style.display = 'none';

    // Determine animation direction BEFORE updating currentPage
    const goingForward = page > currentPage;

    pagedImage.onload = function() {
        loading.style.display = 'none';
        pagedImage.style.display = 'block';
        updateProgress();
        saveProgress();
        preloadImages();
        // Update URL to reflect current page
        const newUrl = `/reader/${TITLE_ID}/${ENTRY_ID}/${currentPage}`;
        history.replaceState(null, '', newUrl);
        document.title = `${document.title.split(' - Page')[0]} - Page ${currentPage}`;
    };

    pagedImage.onerror = function() {
        loading.textContent = 'Error loading page';
        loading.style.color = '#ff6b6b';
    };

    // Add flip animation if enabled
    if (enableAnimation) {
        pagedImage.className = '';
        setTimeout(() => {
            pagedImage.className = goingForward ? 'slide-left' : 'slide-right';
        }, 10);
    }

    // Update current page before loading
    currentPage = page;
    pagedImage.src = getPageUrl(page);
}

// Navigation functions
function nextPage() {
    if (currentPage < TOTAL_PAGES) {
        loadPage(currentPage + 1);
    } else if (NEXT_ENTRY_URL) {
        window.location.href = NEXT_ENTRY_URL;
    }
}

function previousPage() {
    if (currentPage > 1) {
        loadPage(currentPage - 1);
    }
}

// Click zone handlers (respect RTL)
function handleLeftClick() {
    if (enableRTL) {
        nextPage();
    } else {
        previousPage();
    }
}

function handleRightClick() {
    if (enableRTL) {
        previousPage();
    } else {
        nextPage();
    }
}

// Settings functions
function changeMode() {
    mode = modeSelect.value;
    document.body.className = mode === 'continuous' ? 'continuous-mode' : `paged-mode fit-${fit}`;

    // When switching from continuous to paged, get the current page from URL
    // (continuous mode updates the URL as you scroll)
    if (mode === 'paged') {
        const urlParts = window.location.pathname.split('/');
        const pageFromUrl = parseInt(urlParts[urlParts.length - 1]);
        if (pageFromUrl && pageFromUrl !== currentPage) {
            currentPage = pageFromUrl;
        }
    }

    // Show/hide relevant sections
    if (mode === 'continuous') {
        fitSection.classList.add('uk-hidden');
        marginSection.classList.remove('uk-hidden');
        animationSection.classList.add('uk-hidden');
        preloadSection.classList.add('uk-hidden');
        rtlSection.classList.add('uk-hidden');
        initContinuousMode();
    } else {
        fitSection.classList.remove('uk-hidden');
        marginSection.classList.add('uk-hidden');
        animationSection.classList.remove('uk-hidden');
        preloadSection.classList.remove('uk-hidden');
        rtlSection.classList.remove('uk-hidden');
        initPagedMode();
    }

    savePreferences();
}

function changeFit() {
    fit = fitSelect.value;
    document.body.className = `paged-mode fit-${fit}`;
    savePreferences();
}

function changeMargin() {
    margin = parseInt(marginRange.value);
    marginValue.textContent = margin;
    document.querySelectorAll('.continuous-image').forEach(img => {
        img.style.marginTop = `${margin}px`;
        img.style.marginBottom = `${margin}px`;
    });
    savePreferences();
}

function toggleAnimation() {
    enableAnimation = document.getElementById('enable-flip-animation').checked;
    savePreferences();
}

function changePreload() {
    preloadCount = parseInt(preloadRange.value);
    preloadValue.textContent = preloadCount;
    preloadImages();
    savePreferences();
}

function toggleRTL() {
    enableRTL = document.getElementById('enable-rtl').checked;
    savePreferences();
}

function jumpToPage() {
    const page = parseInt(pageSelect.value);
    if (mode === 'paged') {
        loadPage(page);
    } else {
        const img = continuousContainer.querySelector(`[data-page="${page}"]`);
        if (img) {
            img.scrollIntoView({ behavior: 'instant', block: 'start' });
        }
    }
}

function jumpToEntry() {
    const entryId = document.getElementById('entry-select').value;
    window.location.href = `/reader/${TITLE_ID}/${entryId}/1`;
}

// Keyboard shortcuts
document.addEventListener('keydown', function(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;

    switch(e.key) {
        case 'ArrowLeft':
            e.preventDefault();
            handleLeftClick();
            break;
        case 'ArrowRight':
            e.preventDefault();
            handleRightClick();
            break;
        case ' ':
            e.preventDefault();
            nextPage();
            break;
        case 's':
        case 'S':
            e.preventDefault();
            UIkit.modal('#settings-modal').show();
            break;
        case 'Escape':
            UIkit.modal('#settings-modal').hide();
            break;
    }
});

// Save progress when page is being unloaded
// Only save if we're actually reading (not on initial page load)
let hasInteracted = false;
window.addEventListener('beforeunload', function(e) {
    // Only save if user has interacted with the page
    if (hasInteracted) {
        saveProgressBeacon();
    }
});

// Mark as interacted when scrolling or changing pages
window.addEventListener('scroll', function() {
    hasInteracted = true;
}, { once: true });

// Also mark as interacted on page navigation in paged mode
const originalLoadPage = loadPage;
loadPage = function(page) {
    hasInteracted = true;
    return originalLoadPage(page);
};

// Also save on visibility change (mobile Safari, etc.)
document.addEventListener('visibilitychange', function() {
    if (document.visibilityState === 'hidden' && hasInteracted) {
        saveProgressBeacon();
    }
});

// Initialize
loadPreferences();
changeMode();
