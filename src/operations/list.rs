use anyhow::Result;

use crate::database::{self, Package};

pub fn list_packages() -> Result<Vec<Package>> {
    let conn = database::connect()?;
    database::migrate(&conn)?;
    database::list_packages(&conn)
}
