//! Identifier types.

use std::fmt;
use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// A fresh time-ordered (v7) id.
            pub fn generate() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// First 8 hex digits, for display.
            pub fn short(&self) -> String {
                self.0.simple().to_string()[..8].to_string()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.short())
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::from_str(s).map(Self)
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.0.as_bytes().to_vec()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                let b = value.as_blob()?;
                Uuid::from_slice(b).map(Self).map_err(|e| FromSqlError::Other(Box::new(e)))
            }
        }
    };
}

uuid_id!(
    /// Identifies one microversion (one committed document state).
    MicroversionId
);
uuid_id!(
    /// Identifies a branch (Onshape: workspace).
    BranchId
);
uuid_id!(
    /// Identifies a named version.
    VersionId
);

/// blake3 hash of a blob's uncompressed content.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlobHash(pub [u8; 32]);

impl BlobHash {
    pub fn of(data: &[u8]) -> BlobHash {
        BlobHash(*blake3::hash(data).as_bytes())
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlobHash({})", &self.to_hex()[..12])
    }
}

impl ToSql for BlobHash {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(&self.0[..]))
    }
}

impl FromSql for BlobHash {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let b = value.as_blob()?;
        let arr: [u8; 32] = b.try_into().map_err(|_| FromSqlError::InvalidBlobSize {
            expected_size: 32,
            blob_size: b.len(),
        })?;
        Ok(BlobHash(arr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_parse_and_order() {
        let a = MicroversionId::generate();
        let b = MicroversionId::generate();
        assert!(a < b, "v7 ids are time ordered");
        assert_eq!(a.to_string().parse::<MicroversionId>().unwrap(), a);
        assert_eq!(a.short().len(), 8);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, format!("\"{a}\""));
    }

    #[test]
    fn hash() {
        let h = BlobHash::of(b"hello");
        assert_eq!(h, BlobHash::of(b"hello"));
        assert_ne!(h, BlobHash::of(b"hello!"));
        assert_eq!(h.to_hex().len(), 64);
    }
}
