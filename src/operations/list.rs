use anyhow::Result;

use crate::database;
use crate::models::Package;

pub fn list_packages() -> Result<Vec<Package>> {
    let conn = database::lock_conn()?;
    database::list_packages(&conn)
}
