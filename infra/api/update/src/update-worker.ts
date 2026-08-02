type UpdateMode = 'current' | 'full' | 'patch';

interface UpdatePlanRow {
	current_hash: string;
	mode: UpdateMode;
	target_hash: string;
	snapshot_url: string | null;
	patch_urls_json: string | null;
	is_latest_full: number;
	is_stale: number;
	created_at: string;
}

interface UpdatePlanResponse {
	mode: UpdateMode;
	current: string;
	target: string;
	snapshot?: string | null;
	patches: string[];
}

const UPDATE_CACHE_HEADERS = {
	'Cache-Control': 'public, max-age=60',
	'CDN-Cache-Control': 'public, max-age=300',
};

const JSON_HEADERS = {
	'Content-Type': 'application/json; charset=utf-8',
};

export default {
	async fetch(request, env, _ctx): Promise<Response> {
		const url = new URL(request.url);

		if (url.pathname !== '/v1/update') {
			return jsonError('not found', 404);
		}

		if (request.method !== 'GET') {
			return jsonError('method not allowed', 405, { Allow: 'GET' });
		}

		const current = url.searchParams.get('current');

		if (current !== null && current.trim().length === 0) {
			return jsonError('current must not be empty', 400);
		}

		try {
			const plan = current
				? await selectPlanForCurrent(env.DB, current.trim())
				: await selectLatestFullPlan(env.DB);

			return Response.json(plan, {
				headers: UPDATE_CACHE_HEADERS,
			});
		} catch (error) {
			console.error('failed to build update plan', error);
			return jsonError('failed to generate update plan', 500);
		}
	},
} satisfies ExportedHandler<Env>;

async function selectPlanForCurrent(db: D1Database, currentHash: string): Promise<UpdatePlanResponse> {
	const row = await db
		.prepare(
			`SELECT current_hash, mode, target_hash, snapshot_url, patch_urls_json, is_latest_full, is_stale, created_at
			 FROM update_plans
			 WHERE current_hash = ?
			 LIMIT 1`,
		)
		.bind(currentHash)
		.first<UpdatePlanRow>();

	if (!row) {
		return selectLatestFullPlan(db);
	}

	if (row.is_stale !== 0) {
		return selectLatestFullPlan(db);
	}

	try {
		const plan = mapRowToResponse(row);
		// `WHERE current_hash = ?` guarantees row.current_hash === currentHash,
		// so this is a no-op for 'current'/'patch' modes -- but it matters for
		// 'full': mapRowToResponse reports current === target for a 'full' row
		// by default (correct for selectLatestFullPlan's synthetic "latest
		// snapshot" row, whose current_hash is just a row key, not a real
		// client version). Here the row was matched by the client's actual
		// current hash, so that's what should be echoed back, not the target
		// they're upgrading to.
		return { ...plan, current: currentHash };
	} catch (error) {
		console.warn('update plan row is invalid; falling back to latest full snapshot', error);
		return selectLatestFullPlan(db);
	}
}

async function selectLatestFullPlan(db: D1Database): Promise<UpdatePlanResponse> {
	const row = await db
		.prepare(
			`SELECT current_hash, mode, target_hash, snapshot_url, patch_urls_json, is_latest_full, is_stale, created_at
			 FROM update_plans
			 WHERE is_latest_full = 1 AND mode = 'full'
			 ORDER BY created_at DESC
			 LIMIT 1`,
		)
		.first<UpdatePlanRow>();

	if (!row) {
		throw new Error('latest full snapshot is not available');
	}

	return mapRowToResponse(row);
}

function mapRowToResponse(row: UpdatePlanRow): UpdatePlanResponse {
	switch (row.mode) {
		case 'current': {
			return {
				mode: 'current',
				current: row.current_hash,
				target: row.target_hash,
				snapshot: null,
				patches: [],
			};
		}
		case 'full': {
			if (!row.snapshot_url) {
				throw new Error(`full plan ${row.current_hash} is missing a snapshot URL`);
			}

			// Defaults to current === target: correct for
			// selectLatestFullPlan's row, whose current_hash is a synthetic
			// row key (see e.g. seedPlan's `full:sha256:latest` fixture), not
			// a real client version. selectPlanForCurrent overrides `current`
			// to the actual client-supplied hash after calling this, since
			// there the row was matched specifically by that hash.
			return {
				mode: 'full',
				current: row.target_hash,
				target: row.target_hash,
				snapshot: row.snapshot_url,
				patches: [],
			};
		}
		case 'patch': {
			if (!row.patch_urls_json) {
				throw new Error(`patch plan ${row.current_hash} is missing patch URLs`);
			}

			let patches: unknown;

			try {
				patches = JSON.parse(row.patch_urls_json);
			} catch (error) {
				throw new Error(`patch plan ${row.current_hash} has invalid patch JSON`, {
					cause: error,
				});
			}

			if (!Array.isArray(patches) || patches.some((patch) => typeof patch !== 'string')) {
				throw new Error(`patch plan ${row.current_hash} has invalid patch URLs`);
			}

			return {
				mode: 'patch',
				current: row.current_hash,
				target: row.target_hash,
				snapshot: null,
				patches,
			};
		}
	}
}

function jsonError(message: string, status: number, extraHeaders?: HeadersInit): Response {
	return Response.json(
		{ error: message },
		{
			status,
			headers: {
				...JSON_HEADERS,
				...extraHeaders,
			},
		},
	);
}
