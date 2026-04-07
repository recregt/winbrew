use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use winbrew_models::RawFetchedPackage;

use crate::error::ParserError;
use crate::parser::{ParsedPackage, parse_package};

const QUERY: &str = r#"
SELECT
    i.id,
    n.name,
    v.version,
    np.norm_publisher
FROM manifest m
JOIN ids i        ON i.rowid = m.id
JOIN names n      ON n.rowid = m.name
JOIN versions v   ON v.rowid = m.version
LEFT JOIN norm_publishers_map npm ON npm.manifest = m.rowid
LEFT JOIN norm_publishers np      ON np.rowid = npm.norm_publisher
GROUP BY i.id
HAVING v.version = MAX(v.version)
ORDER BY i.id ASC
"#;

pub fn read_winget_packages<F>(path: &Path, mut on_package: F) -> Result<(), ParserError>
where
    F: FnMut(ParsedPackage) -> Result<(), ParserError>,
{
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(QUERY)?;
    let mut rows = statement.query([])?;

    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let version: String = row.get(2)?;
        let publisher: Option<String> = row.get(3)?;

        let raw = RawFetchedPackage {
            id: format!("winget/{id}"),
            name,
            version,
            description: None,
            homepage: None,
            license: None,
            publisher: publisher.and_then(|publisher| {
                let trimmed = publisher.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }),
            installers: Vec::new(),
        };

        match parse_package(raw) {
            Ok(parsed) => on_package(parsed)?,
            Err(err) => eprintln!("skipping winget package {id}: {err}"),
        }
    }

    Ok(())
}
