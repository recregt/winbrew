# Parser Fixtures

These JSONL files are generated from real upstream data through the crawler package tests.

To refresh them from the repository root:

```powershell
Set-Location infra/crawler
$env:WINBREW_REFRESH_PARSER_FIXTURES='1'
go test ./pkg/sources/scoop ./pkg/sources/winget -run TestRefreshParserFixtures -count=1
Remove-Item Env:WINBREW_REFRESH_PARSER_FIXTURES
```

The generated files are:

- `scoop_packages.jsonl`
- `winget_packages.jsonl`
