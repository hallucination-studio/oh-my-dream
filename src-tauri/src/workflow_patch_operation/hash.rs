use serde::Serialize;

pub(super) fn request_hash<T: Serialize + ?Sized>(
    expected_revision: Option<u64>,
    patch: &T,
) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(&(expected_revision, patch))?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("fnv1a:{hash:016x}"))
}
