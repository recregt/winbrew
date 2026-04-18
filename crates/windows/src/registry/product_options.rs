use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

const PRODUCT_OPTIONS_KEY: &str = r"SYSTEM\CurrentControlSet\Control\ProductOptions";
const PRODUCT_TYPE_VALUE: &str = "ProductType";

/// Read the Windows product type when it is available.
pub(crate) fn read_product_type() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let Ok(product_options_key) = hklm.open_subkey(PRODUCT_OPTIONS_KEY) else {
        return None;
    };

    let Ok(product_type) = product_options_key.get_value::<String, _>(PRODUCT_TYPE_VALUE) else {
        return None;
    };

    let product_type = product_type.trim();
    if product_type.is_empty() {
        None
    } else {
        Some(product_type.to_string())
    }
}
