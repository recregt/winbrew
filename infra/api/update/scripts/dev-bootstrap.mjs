import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const packageDir = dirname(scriptDir);
const devVarsPath = join(packageDir, '.dev.vars');
const generatedConfigPath = join(packageDir, 'wrangler.dev.jsonc');
const wranglerBinPath = join(packageDir, 'node_modules', 'wrangler', 'bin', 'wrangler.js');

const devVars = readDevVars(devVarsPath);
const databaseName = devVars.WINBREW_UPDATE_DB_NAME?.trim() || 'winbrew-update';
const databaseId = devVars.WINBREW_UPDATE_DB_ID?.trim() || '';
const previewDatabaseId = devVars.WINBREW_UPDATE_PREVIEW_DB_ID?.trim() || '';

writeFileSync(
	generatedConfigPath,
	JSON.stringify(
		{
			$schema: 'node_modules/wrangler/config-schema.json',
			name: 'update',
			main: 'src/index.ts',
			compatibility_date: '2026-04-16',
			d1_databases: [
				{
					binding: 'DB',
					database_name: databaseName,
					...(databaseId ? { database_id: databaseId } : {}),
					...(previewDatabaseId ? { preview_database_id: previewDatabaseId } : {}),
				},
			],
			observability: {
				enabled: true,
			},
			upload_source_maps: true,
			compatibility_flags: ['nodejs_compat'],
		},
		null,
		2,
	) + '\n',
);

runWrangler('d1', 'execute', databaseName, '--local', '--file=./migrations/0001_init.sql', '--yes');
runWrangler('d1', 'execute', databaseName, '--local', '--file=./seed/local-dev.sql', '--yes');

function readDevVars(filePath) {
	const variables = {};

	for (const rawLine of readFileSync(filePath, 'utf8').split(/\r?\n/)) {
		const line = rawLine.trim();
		if (!line || line.startsWith('#')) {
			continue;
		}

		const separatorIndex = line.indexOf('=');
		if (separatorIndex < 0) {
			continue;
		}

		const key = line.slice(0, separatorIndex).trim();
		const value = line.slice(separatorIndex + 1).trim();
		variables[key] = unquote(value);
	}

	return variables;
}

function unquote(value) {
	if (value.length >= 2) {
		const firstCharacter = value[0];
		const lastCharacter = value[value.length - 1];
		if ((firstCharacter === '"' && lastCharacter === '"') || (firstCharacter === '\'' && lastCharacter === '\'')) {
			return value.slice(1, -1);
		}
	}

	return value;
}

function runWrangler(...args) {
	const result = spawnSync(process.execPath, [wranglerBinPath, ...args], {
		cwd: packageDir,
		stdio: 'inherit',
	});

	if (result.error) {
		console.error(result.error);
		process.exit(1);
	}

	if (result.status !== 0) {
		process.exit(result.status ?? 1);
	}
}