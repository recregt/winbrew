import {
	env,
	createExecutionContext,
	waitOnExecutionContext,
	SELF,
} from 'cloudflare:test';
import { beforeEach, describe, expect, it } from 'vitest';
import worker from '../src/index';

const IncomingRequest = Request<unknown, IncomingRequestCfProperties>;

const SCHEMA_STATEMENTS = [
	'DROP TABLE IF EXISTS schema_meta;',
	'DROP TABLE IF EXISTS release_lineage;',
	'DROP TABLE IF EXISTS patch_artifacts;',
	'DROP TABLE IF EXISTS update_plans;',
	'CREATE TABLE schema_meta (id INTEGER PRIMARY KEY CHECK (id = 1), schema_version INTEGER NOT NULL);',
	'CREATE TABLE release_lineage (hash TEXT PRIMARY KEY, parent_hash TEXT, is_snapshot INTEGER NOT NULL DEFAULT 0, snapshot_url TEXT, metadata_url TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);',
	'CREATE TABLE patch_artifacts (from_hash TEXT NOT NULL, to_hash TEXT NOT NULL, file_path TEXT NOT NULL, size_bytes INTEGER NOT NULL, checksum TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (from_hash, to_hash));',
	"CREATE TABLE update_plans (current_hash TEXT PRIMARY KEY, mode TEXT NOT NULL CHECK (mode IN ('current', 'full', 'patch')), target_hash TEXT NOT NULL, snapshot_url TEXT, patch_urls_json TEXT NOT NULL DEFAULT '[]', chain_length INTEGER NOT NULL DEFAULT 0, total_patch_bytes INTEGER NOT NULL DEFAULT 0, is_latest_full INTEGER NOT NULL DEFAULT 0, is_stale INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
];

async function resetDatabase() {
	for (const statement of SCHEMA_STATEMENTS) {
		await env.DB.exec(statement);
	}
}

async function seedPlan(row: {
	currentHash: string;
	mode: 'current' | 'full' | 'patch';
	targetHash: string;
	snapshotUrl?: string | null;
	patchUrls?: string[];
	chainLength?: number;
	totalPatchBytes?: number;
	isLatestFull?: boolean;
	isStale?: boolean;
}) {
	await env.DB
		.prepare(
			`INSERT INTO update_plans (
				current_hash, mode, target_hash, snapshot_url, patch_urls_json, chain_length, total_patch_bytes, is_latest_full, is_stale
			) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		)
		.bind(
			row.currentHash,
			row.mode,
			row.targetHash,
			row.snapshotUrl ?? null,
			JSON.stringify(row.patchUrls ?? []),
			row.chainLength ?? 0,
			row.totalPatchBytes ?? 0,
			row.isLatestFull ? 1 : 0,
			row.isStale ? 1 : 0,
		)
		.run();
}

beforeEach(async () => {
	await resetDatabase();
});

describe('update worker', () => {
	it('returns 404 for non-update paths', async () => {
		const response = await SELF.fetch('https://api.winbrew.dev/');

		expect(response.status).toBe(404);
		expect(await response.json()).toEqual({ error: 'not found' });
	});

	it('returns the current state without downloading when the client is current', async () => {
		await seedPlan({
			currentHash: 'sha256:current',
			mode: 'current',
			targetHash: 'sha256:current',
		});
		await seedPlan({
			currentHash: 'sha256:latest',
			mode: 'full',
			targetHash: 'sha256:latest',
			snapshotUrl: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			isLatestFull: true,
		});

		const request = new IncomingRequest('https://api.winbrew.dev/v1/update?current=sha256:current');
		const ctx = createExecutionContext();
		const response = await worker.fetch(request, env, ctx);
		await waitOnExecutionContext(ctx);

		expect(response.status).toBe(200);
		expect(await response.json()).toEqual({
			mode: 'current',
			current: 'sha256:current',
			target: 'sha256:current',
			snapshot: null,
			patches: [],
		});
	});

	it('returns the latest full snapshot when no client hash is provided', async () => {
		await seedPlan({
			currentHash: 'sha256:latest',
			mode: 'full',
			targetHash: 'sha256:latest',
			snapshotUrl: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			isLatestFull: true,
		});

		const response = await SELF.fetch('https://api.winbrew.dev/v1/update');
		expect(response.status).toBe(200);
		expect(await response.json()).toEqual({
			mode: 'full',
			current: 'sha256:latest',
			target: 'sha256:latest',
			snapshot: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			patches: [],
		});
	});

	it('returns the latest full snapshot target hash even when the row key differs', async () => {
		await seedPlan({
			currentHash: 'full:sha256:latest',
			mode: 'full',
			targetHash: 'sha256:latest',
			snapshotUrl: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			isLatestFull: true,
		});

		const response = await SELF.fetch('https://api.winbrew.dev/v1/update');
		expect(response.status).toBe(200);
		expect(await response.json()).toEqual({
			mode: 'full',
			current: 'sha256:latest',
			target: 'sha256:latest',
			snapshot: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			patches: [],
		});
	});

	it('returns patch chains for recent clients', async () => {
		await seedPlan({
			currentHash: 'sha256:current',
			mode: 'patch',
			targetHash: 'sha256:next',
			patchUrls: [
				'https://cdn.winbrew.dev/patches/001.sql.zst',
				'https://cdn.winbrew.dev/patches/002.sql.zst',
			],
			chainLength: 2,
			totalPatchBytes: 1024,
		});
		await seedPlan({
			currentHash: 'sha256:latest',
			mode: 'full',
			targetHash: 'sha256:latest',
			snapshotUrl: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			isLatestFull: true,
		});

		const response = await SELF.fetch('https://api.winbrew.dev/v1/update?current=sha256:current');
		expect(response.status).toBe(200);
		expect(await response.json()).toEqual({
			mode: 'patch',
			current: 'sha256:current',
			target: 'sha256:next',
			snapshot: null,
			patches: [
				'https://cdn.winbrew.dev/patches/001.sql.zst',
				'https://cdn.winbrew.dev/patches/002.sql.zst',
			],
		});
	});

	it('falls back to the latest full snapshot when the current plan is stale', async () => {
		await seedPlan({
			currentHash: 'sha256:stale',
			mode: 'patch',
			targetHash: 'sha256:next',
			patchUrls: ['https://cdn.winbrew.dev/patches/001.sql.zst'],
			isStale: true,
		});
		await seedPlan({
			currentHash: 'sha256:latest',
			mode: 'full',
			targetHash: 'sha256:latest',
			snapshotUrl: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			isLatestFull: true,
		});

		const response = await SELF.fetch('https://api.winbrew.dev/v1/update?current=sha256:stale');
		expect(response.status).toBe(200);
		expect(await response.json()).toEqual({
			mode: 'full',
			current: 'sha256:latest',
			target: 'sha256:latest',
			snapshot: 'https://cdn.winbrew.dev/catalog/latest.db.zst',
			patches: [],
		});
	});

	it('rejects blank current values', async () => {
		const response = await SELF.fetch('https://api.winbrew.dev/v1/update?current=');
		expect(response.status).toBe(400);
		expect(await response.json()).toEqual({ error: 'current must not be empty' });
	});
});
