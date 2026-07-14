pub(crate) const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A_64_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) struct FixtureDigest {
    hash: u64,
}

impl FixtureDigest {
    pub(crate) const fn new() -> Self {
        Self {
            hash: FNV1A_64_OFFSET_BASIS,
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(FNV1A_64_PRIME);
        }
    }

    pub(crate) fn finish(&self) -> String {
        format!("{:016x}", self.hash)
    }
}

pub(crate) fn fixture_digest(bytes: &[u8]) -> String {
    let mut digest = FixtureDigest::new();
    digest.update(bytes);
    digest.finish()
}

#[cfg(test)]
mod tests {
    use super::fixture_digest;

    #[test]
    fn fnv1a_fixture_digest_has_known_vectors() {
        assert_eq!(fixture_digest(b""), "cbf29ce484222325");
        assert_eq!(fixture_digest(b"hello"), "a430d84680aabd0b");
    }
}
