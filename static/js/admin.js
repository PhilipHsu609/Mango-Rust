// Alpine.js component for admin page
function adminData() {
    return {
        progress: 1.0,
        generating: false,
        scanning: false,
        scanTitles: 0,
        scanMs: -1,

        // Scan library files (TODO: Implement API endpoint)
        async scanLibrary() {
            if (this.scanning) return;

            this.scanning = true;
            this.scanMs = -1;
            this.scanTitles = 0;

            try {
                const response = await fetch('/api/admin/scan', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    }
                });

                if (!response.ok) {
                    throw new Error('Scan API not yet implemented');
                }

                const data = await response.json();
                this.scanMs = data.milliseconds;
                this.scanTitles = data.titles;
            } catch (error) {
                console.log('Scan library feature coming soon');
                alert('Scan library feature is not yet implemented');
            } finally {
                this.scanning = false;
            }
        },

        // Generate thumbnails (TODO: Implement API endpoint)
        async generateThumbnails() {
            if (this.generating) return;

            this.generating = true;
            this.progress = 0.0;

            try {
                const response = await fetch('/api/admin/generate_thumbnails', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    }
                });

                if (!response.ok) {
                    throw new Error('Generate thumbnails API not yet implemented');
                }

                // Start polling for progress
                this.checkProgress();
            } catch (error) {
                console.log('Generate thumbnails feature coming soon');
                alert('Generate thumbnails feature is not yet implemented');
                this.generating = false;
            }
        },

        // Check thumbnail generation progress (TODO: Implement API endpoint)
        async checkProgress() {
            try {
                const response = await fetch('/api/admin/thumbnail_progress');

                if (response.ok) {
                    const data = await response.json();
                    this.progress = data.progress;
                    this.generating = data.progress > 0 && data.progress < 1.0;

                    // Continue polling if still generating
                    if (this.generating) {
                        setTimeout(() => this.checkProgress(), 1000);
                    }
                } else {
                    this.generating = false;
                }
            } catch (error) {
                this.generating = false;
            }
        }
    };
}
