use anyhow::Result;

use crate::database::{self, Package};

pub fn list_packages() -> Result<Vec<Package>> {
    let conn = database::lock_conn()?;
    database::list_packages(&conn)
}
