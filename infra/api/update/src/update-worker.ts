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
		if (request.method !== 'GET') {
			return jsonError('method not allowed', 405, { Allow: 'GET' });
		}

		const url = new URL(request.url);
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
		return mapRowToResponse(row);
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

			return {
				mode: 'full',
				current: row.current_hash,
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
