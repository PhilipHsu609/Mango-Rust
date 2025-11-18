/**
 * Alpine.js component for the home page
 * Fetches data from three API endpoints and manages reactive state
 * Also handles modal behavior for entry cards
 */
function homePageData() {
    return {
        continueReading: [],
        startReading: [],
        recentlyAdded: [],
        loading: true,
        error: null,

        // Current modal state
        currentEntry: null,

        async init() {
            await this.loadAllSections();
        },

        async loadAllSections() {
            try {
                // Fetch all three sections in parallel
                const [continueRes, startRes, recentRes] = await Promise.all([
                    fetch('/api/library/continue_reading'),
                    fetch('/api/library/start_reading'),
                    fetch('/api/library/recently_added')
                ]);

                if (continueRes.ok) {
                    this.continueReading = await continueRes.json();
                }

                if (startRes.ok) {
                    this.startReading = await startRes.json();
                }

                if (recentRes.ok) {
                    this.recentlyAdded = await recentRes.json();
                }

                this.loading = false;
            } catch (err) {
                console.error('Failed to load home page data:', err);
                this.error = err.message;
                this.loading = false;
            }
        },

        showModal(titleId, titleName, entryId, entryName, pages, percentage, progress) {
            // Store current entry data
            this.currentEntry = {
                titleId,
                titleName,
                entryId,
                entryName,
                pages,
                percentage: parseFloat(percentage) || 0,
                progress: progress || 0
            };

            // Update modal content
            document.getElementById('modal-title').textContent = entryName;
            document.getElementById('modal-pages').textContent = `${pages} pages`;

            // Set up "From beginning" button
            const beginningBtn = document.getElementById('modal-beginning-btn');
            beginningBtn.href = `/reader/${titleId}/${entryId}/1`;

            // Set up "Continue" button
            const continueBtn = document.getElementById('modal-continue-btn');
            if (this.currentEntry.progress > 0) {
                continueBtn.textContent = `Continue from ${this.currentEntry.percentage.toFixed(1)}%`;
                // Use progress value as the page number (progress is the current page)
                continueBtn.href = `/reader/${titleId}/${entryId}/${this.currentEntry.progress}`;
                continueBtn.style.display = '';
            } else {
                continueBtn.style.display = 'none';
            }

            // Show modal using UIKit
            UIkit.modal('#entry-modal').show();
        },

        async markAsRead() {
            if (!this.currentEntry) return;

            try {
                const response = await fetch(
                    `/api/progress/${this.currentEntry.titleId}/${this.currentEntry.entryId}`,
                    {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({
                            page: this.currentEntry.pages
                        })
                    }
                );

                if (response.ok) {
                    // Close modal
                    UIkit.modal('#entry-modal').hide();
                    // Reload data to reflect changes
                    await this.loadAllSections();
                } else {
                    console.error('Failed to mark as read');
                }
            } catch (err) {
                console.error('Error marking as read:', err);
            }
        },

        async markAsUnread() {
            if (!this.currentEntry) return;

            try {
                const response = await fetch(
                    `/api/progress/${this.currentEntry.titleId}/${this.currentEntry.entryId}`,
                    {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({
                            page: 0
                        })
                    }
                );

                if (response.ok) {
                    // Close modal
                    UIkit.modal('#entry-modal').hide();
                    // Reload data to reflect changes
                    await this.loadAllSections();
                } else {
                    console.error('Failed to mark as unread');
                }
            } catch (err) {
                console.error('Error marking as unread:', err);
            }
        }
    };
}
