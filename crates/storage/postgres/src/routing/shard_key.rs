use std::hash::Hasher;

/// Contract for values that can deterministically identify a database shard.
///
/// ## Why not `fn shard_bytes(&self) -> &[u8]`?
///
/// That signature works for heap-backed types (`String`, `Uuid`) but fails for
/// stack-allocated primitives: `u64::to_le_bytes()` produces a `[u8; 8]` that
/// is dropped at the end of the method, so a reference to it would dangle.
/// Encoding the value as a temporary and returning a reference to it is
/// impossible without either a heap allocation or a hidden lifetime on `Self`.
///
/// ## The feed pattern
///
/// Instead, this trait mirrors `std::hash::Hash`: implementors push their
/// canonical byte representation into a generic `Hasher` via its `write_*`
/// methods.  The routing layer constructs the hasher, passes a mutable
/// reference in, and calls `finish()` — entirely zero-allocation.
///
/// ## Implementing
///
/// Use `state.write(bytes)` for borrowed byte slices, or the typed methods
/// (`write_u64`, `write_u128`, …) for numeric values.  The only invariant that
/// must hold: **two logically equal keys must feed identical bytes**.
///
/// ```rust
/// use std::hash::Hasher;
/// use postgres::ShardKey;
///
/// struct AccountId(u64);
///
/// impl ShardKey for AccountId {
///     fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
///         state.write_u64(self.0);
///     }
/// }
/// ```
pub trait ShardKey {
    fn hash_shard_key<H: Hasher>(&self, state: &mut H);
}

impl ShardKey for uuid::Uuid {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        // as_bytes() returns &[u8; 16] — a reference into self, no allocation.
        state.write(self.as_bytes());
    }
}

impl ShardKey for String {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes());
    }
}

impl ShardKey for str {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes());
    }
}

impl ShardKey for u64 {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        // write_u64 feeds 8 bytes directly into the hasher — zero allocation.
        state.write_u64(*self);
    }
}

impl ShardKey for u128 {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write_u128(*self);
    }
}

impl ShardKey for i64 {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write_i64(*self);
    }
}

impl ShardKey for [u8] {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write(self);
    }
}

impl<const N: usize> ShardKey for [u8; N] {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_slice());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seahash::SeaHasher;

    fn hash_key<K: ShardKey + ?Sized>(key: &K) -> u64 {
        let mut h = SeaHasher::new();
        key.hash_shard_key(&mut h);
        h.finish()
    }

    #[test]
    fn uuid_is_deterministic() {
        let id = uuid::Uuid::new_v4();
        assert_eq!(hash_key(&id), hash_key(&id));
    }

    #[test]
    fn u64_is_deterministic_and_zero_alloc() {
        assert_eq!(hash_key(&42u64), hash_key(&42u64));
        assert_ne!(hash_key(&42u64), hash_key(&43u64));
    }

    #[test]
    fn string_and_str_produce_same_hash() {
        let owned = String::from("account-key");
        let borrowed: &str = "account-key";
        assert_eq!(hash_key(&*owned), hash_key(borrowed));
    }

    #[test]
    fn byte_array_and_slice_produce_same_hash() {
        let arr: [u8; 4] = [1, 2, 3, 4];
        let slice: &[u8] = &arr;
        assert_eq!(hash_key(&arr), hash_key(slice));
    }
}
