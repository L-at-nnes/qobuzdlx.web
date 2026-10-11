// The shapes the server speaks. They mirror the Rust side (src/web, src/download, src/settings).

export type Language = 'en' | 'fr';

export interface Account {
	id: number;
	display_name: string | null;
	email: string | null;
	plan: string | null;
}

export interface AccessStatus {
	protected: boolean;
	unlocked: boolean;
	managed: boolean;
	locked_out_seconds: number;
}

export interface SessionInfo {
	signed_in: boolean;
	account: Account | null;
}

// ─── Catalogue ───

export interface AlbumItem {
	id: string;
	url: string;
	title: string;
	version: string | null;
	artist: string;
	release_date: string | null;
	tracks_count: number;
	bit_depth: number;
	sampling_rate: number;
	label: string;
	thumbnail: string;
	parental_warning: boolean;
}

export interface TrackItem {
	id: number;
	url: string;
	title: string;
	version: string | null;
	number: number;
	disc: number;
	artist: string;
	album: { id: string; title: string; thumbnail: string; release_date: string | null } | null;
	duration: number;
	bit_depth: number;
	sampling_rate: number;
	parental_warning: boolean;
}

export interface ArtistItem {
	id: number;
	url: string;
	name: string;
	picture: string | null;
	albums_count: number;
}

export interface PlaylistItem {
	id: number;
	url: string;
	name: string;
	owner: string | null;
	tracks_count: number;
	image: string | null;
}

export interface Results<T> {
	total: number;
	offset: number;
	items: T[];
}

export type SearchKind = 'albums' | 'tracks' | 'artists' | 'playlists';

// ─── Downloads ───

export type ItemStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface FailedTrack {
	track_id: number;
	title: string;
	error: string;
}

export interface QueueItem {
	id: string;
	url: string;
	kind: string;
	ref_id: string;
	label: string;
	sub_label: string | null;
	thumbnail: string | null;
	status: ItemStatus;
	error: string | null;
	enqueued_at: number;
	started_at: number | null;
	finished_at: number | null;
	downloaded: number;
	skipped: number;
	failures: FailedTrack[];
}

export interface AlbumCard {
	title: string;
	artist: string;
	release_date: string | null;
	tracks_count: number;
	genre: string;
	label: string;
	quality: string;
	cover: string;
}

export interface TrackProgress {
	title: string;
	index: number;
	total: number;
	percent: number;
	bytes_per_second: number;
}

export interface Progress {
	running: boolean;
	item_id: string | null;
	percent: number;
	album: AlbumCard | null;
	/** The tracks being downloaded, by position: an album downloads several at once. */
	tracks: TrackProgress[];
	/** Why the queue stopped by itself, when it did. */
	halt: 'storage_full' | null;
}

export interface Enqueued {
	added: number;
	requeued: number;
	duplicates: number;
	invalid: number;
}

export type LogLevel = 'errors' | 'normal' | 'detailed';

/** One line of the log: an event and its fields, which the interface words in its language. */
export interface LogLine {
	seq: number;
	level: LogLevel;
	event: string;
	[field: string]: unknown;
}

// ─── Settings ───

export type Quality = 'mp3' | 'flac16' | 'flac24' | 'flac24_max';
export type Duplicates = 'skip' | 'rename' | 'overwrite';
export type ArtworkSize = number | 'original';

export interface TitleMarker {
	enabled: boolean;
	text: string;
	position: 'prefix' | 'suffix';
}

export interface Settings {
	language: Language | null;
	download: { directory: string; quality: Quality };
	naming: {
		artist: string;
		album: string;
		track: string;
		compilation_track: string;
		playlist: string;
		disc: string;
		merge_artists: boolean;
		merge_artists_in_folders: boolean;
	};
	tags: {
		album: boolean;
		album_artist: boolean;
		artist: boolean;
		title: boolean;
		track_number: boolean;
		total_tracks: boolean;
		disc_number: boolean;
		total_discs: boolean;
		genre: boolean;
		year: boolean;
		release_date: boolean;
		composer: boolean;
		copyright: boolean;
		label: boolean;
		isrc: boolean;
		upc: boolean;
		release_type: boolean;
		explicit: boolean;
		cover: boolean;
		url: boolean;
		comment: boolean;
		comment_text: string;
		explicit_marker: TitleMarker;
		clean_marker: TitleMarker;
	};
	artwork: { embedded_size: ArtworkSize; saved_size: ArtworkSize; save_to_disk: boolean };
	advanced: {
		streamable_check: boolean;
		verify_integrity: boolean;
		fix_md5: boolean;
		download_goodies: boolean;
		playlist_positions: boolean;
		playlist_file: boolean;
		log_detail: LogLevel;
		duplicates: Duplicates;
	};
}

export type TemplateField =
	| 'artist'
	| 'album'
	| 'track'
	| 'compilation_track'
	| 'playlist'
	| 'disc';

export interface TemplateIssue {
	field: TemplateField;
	kind: 'empty' | 'rooted' | 'parent' | 'illegal_characters' | 'unknown_variable';
	detail: string | null;
}

export interface TemplatePreview {
	preview: { album: string; compilation: string; playlist: string };
	issues: TemplateIssue[];
}

export interface Verdict {
	check: 'download_folder' | 'config_folder' | 'session' | 'credentials';
	status: 'ok' | 'error';
	code: string;
	detail: string | null;
}
