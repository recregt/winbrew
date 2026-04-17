# Parser Fixtures

These JSONL files are generated from real upstream data through the crawler.

To refresh them from the repository root:

```powershell
Set-Location infra/crawler
go run ./cmd/crawler tools generate-fixtures --count=500 --output ..\parser\tests\fixtures\winget_packages.jsonl
$env:WINBREW_REFRESH_PARSER_FIXTURES='1'
go test ./pkg/sources/scoop -run TestRefreshParserFixtures -count=1
Remove-Item Env:WINBREW_REFRESH_PARSER_FIXTURES
```

Use the count that matches the fixture you want to refresh.

The generated files are:

- `scoop_packages.jsonl`
- `winget_packages.jsonl`
