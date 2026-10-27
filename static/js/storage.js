const storageComponent = () => ({
	report: null,
	loading: false,
	refreshing: false,
	scanProgress: null,
	error: '',
	search: '',
	sortKey: 'size',
	descending: true,
	expanded: {},
	// Rendering thousands of rows takes seconds; show the largest first.
	pageSize: 100,
	limit: 100,
	now: Date.now(),

	init() {
		// Keep "measured 2 minutes ago" current without reloading the report.
		setInterval(() => { this.now = Date.now(); }, 30000);
		this.$watch('search', () => { this.limit = this.pageSize; });
		this.loading = true;
		this.loadReport().catch(error => this.showError('Failed to load storage information', error))
			.finally(() => { this.loading = false; });
	},
	async loadReport() {
		const report = await $.get(`${base_url}api/admin/storage`);
		this.report = report;
		this.error = '';
	},
	async refresh() {
		if (this.refreshing || this.loading) return;
		this.refreshing = true;
		this.scanProgress = null;
		this.error = '';
		const timer = setInterval(() => this.pollScan(), 500);
		try {
			await $.post(`${base_url}api/admin/scan`);
			await this.loadReport();
		} catch (error) {
			if (this.report) this.report.stale = true;
			this.showError('Failed to refresh; showing the previous measurement', error);
		} finally {
			clearInterval(timer);
			this.scanProgress = null;
			this.refreshing = false;
		}
	},
	async pollScan() {
		try {
			const progress = await $.get(`${base_url}api/admin/scan_progress`);
			if (this.refreshing && progress.scanning && progress.total > 0) this.scanProgress = progress;
		} catch (error) {
			// Progress is cosmetic; the scan request reports real failures.
		}
	},
	refreshLabel() {
		if (!this.refreshing) return 'Refresh';
		const progress = this.scanProgress;
		return progress ? `Scanning ${progress.completed} / ${progress.total}` : 'Scanning…';
	},
	showError(message, error) {
		const detail = error.responseJSON && error.responseJSON.error
			? error.responseJSON.error : error.statusText || error.message || String(error);
		this.error = `${message}. ${detail}`;
	},
	formatBytes(bytes) {
		const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB', 'EiB'];
		const index = bytes === 0 ? 0 : Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
		return index === 0 ? `${bytes} B` : `${(bytes / Math.pow(1024, index)).toFixed(1)} ${units[index]}`;
	},
	relativeTime(seconds) {
		// Clamp so browser/server clock skew never reads "in 2 minutes".
		const diff = Math.min(0, Math.round((seconds * 1000 - this.now) / 1000));
		if (-diff < 45) return 'just now';
		const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
		if (-diff >= 86400) return rtf.format(Math.trunc(diff / 86400), 'day');
		if (-diff >= 3600) return rtf.format(Math.trunc(diff / 3600), 'hour');
		return rtf.format(Math.min(-1, Math.trunc(diff / 60)), 'minute');
	},
	measuredAt() {
		return this.report && this.report.measured_at !== null
			? new Date(this.report.measured_at * 1000).toLocaleString() : '';
	},
	countTitles(titles) {
		return titles.reduce((sum, title) => sum + 1 + this.countTitles(title.children), 0);
	},
	titles(count) { return `${count} ${count === 1 ? 'title' : 'titles'}`; },
	entries(count) { return `${count} ${count === 1 ? 'entry' : 'entries'}`; },
	hasKnownSize(title) { return title.complete || title.measured_entries > 0; },
	totalSize() {
		if (!this.report) return '—';
		const known = this.report.complete || this.report.titles.some(title => this.hasKnownSize(title));
		if (!known) return 'Not measured';
		return `${this.report.complete ? '' : '≥ '}${this.formatBytes(this.report.total_bytes)}`;
	},
	titleSize(title) {
		if (!this.hasKnownSize(title)) return 'Not measured';
		return `${title.complete ? '' : '≥ '}${this.formatBytes(title.size_bytes)}`;
	},
	summary() {
		if (!this.report) return '';
		const entries = this.report.titles.reduce((sum, title) => sum + title.entry_count, 0);
		const when = this.report.measured_at === null ? 'measurement time unknown'
			: `measured ${this.relativeTime(this.report.measured_at)}`;
		const partial = this.report.complete ? '' : ' · incomplete, sizes are lower bounds';
		return `${this.titles(this.countTitles(this.report.titles))} · ${this.entries(entries)} · ${when}${partial}`;
	},
	subline(title) {
		const entries = this.entries(title.entry_count);
		return title.path && title.path !== title.name ? `${title.path} · ${entries}` : entries;
	},
	share(title) {
		if (!this.report || !this.report.total_bytes || !this.hasKnownSize(title)) return null;
		return Math.min(100, (title.size_bytes / this.report.total_bytes) * 100);
	},
	// Scanner messages are technical; show a short label and keep the raw text as detail.
	describeIssue(issue) {
		const rules = [
			[/^Archive error/i, 'Unreadable archive'],
			[/^Unable to scan region/, 'Could not read folder'],
			[/^Entry size has not been measured/, 'Size not measured yet'],
			[/^Unsafe or unsupported indexed payload/, 'Not counted: unsafe or unsupported file'],
			[/^Overlapping indexed payloads/, 'Not counted: overlaps another entry'],
			[/^Entry identity is shared/, 'Duplicate entry ID'],
			[/^Refresh failed/, 'Last refresh failed'],
			[/^Library scan task failed/, 'Part of the library was not scanned'],
			[/exceeds the representable/, 'Size too large to total'],
			[/permission denied/i, 'Permission denied'],
		];
		const rule = rules.find(([pattern]) => pattern.test(issue.message));
		return { label: rule ? rule[1] : 'Problem', path: issue.path || 'Library root', detail: issue.message };
	},
	problemSummary(issues) {
		const labels = [...new Set(issues.map(issue => this.describeIssue(issue).label))];
		if (labels.length === 1) return issues.length === 1 ? labels[0] : `${issues.length} × ${labels[0]}`;
		return `${issues.length} problems`;
	},
	bookUrl(id) { return `${base_url}book/${encodeURIComponent(id)}`; },
	nestedIds(titles) {
		return titles.flatMap(title => title.children.length ? [title.id, ...this.nestedIds(title.children)] : []);
	},
	hasNested() { return !!this.report && this.report.titles.some(title => title.children.length); },
	allExpanded() {
		const ids = this.report ? this.nestedIds(this.report.titles) : [];
		return ids.length > 0 && ids.every(id => this.expanded[id]);
	},
	toggleAll() {
		this.expanded = this.allExpanded() ? {}
			: Object.fromEntries(this.nestedIds(this.report.titles).map(id => [id, true]));
	},
	toggle(id) { this.expanded = { ...this.expanded, [id]: !this.expanded[id] }; },
	isExpanded(id) { return this.search.trim() !== '' || !!this.expanded[id]; },
	sort(key) {
		if (this.sortKey === key) this.descending = !this.descending;
		else { this.sortKey = key; this.descending = key === 'size'; }
		this.limit = this.pageSize;
	},
	sortLabel(label, key) { return `${label}${this.sortKey === key ? (this.descending ? ' ↓' : ' ↑') : ''}`; },
	visibleRows() { return this.rows().slice(0, this.limit); },
	hiddenRows() { return Math.max(0, this.rows().length - this.limit); },
	showMore() { this.limit += this.pageSize; },
	rows() {
		if (!this.report) return [];
		const query = this.search.trim().toLocaleLowerCase();
		const matches = title => !query || title.name.toLocaleLowerCase().includes(query) || title.path.toLocaleLowerCase().includes(query);
		const compare = (a, b) => {
			// Unknown sizes always come last, not alongside measured zero bytes.
			if (this.sortKey === 'size') {
				const aKnown = this.hasKnownSize(a);
				const bKnown = this.hasKnownSize(b);
				if (aKnown !== bKnown) return aKnown ? -1 : 1;
			}
			const order = this.sortKey === 'size' ? a.size_bytes - b.size_bytes : a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
			return (this.descending ? -order : order) || a.path.localeCompare(b.path) || a.id.localeCompare(b.id);
		};
		// Build matching subtrees once; ancestors survive when a descendant matches.
		const filter = titles => titles.map(title => ({ title, children: filter(title.children) }))
			.filter(node => matches(node.title) || node.children.length).sort((a, b) => compare(a.title, b.title));
		const rows = [];
		const flatten = (nodes, depth) => {
			for (const node of nodes) {
				rows.push({ title: node.title, depth });
				if (this.isExpanded(node.title.id)) flatten(node.children, depth + 1);
			}
		};
		flatten(filter(this.report.titles), 0);
		return rows;
	},
});
