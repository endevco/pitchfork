//! Configuration value types used across pitchfork.toml, daemon state, and CLI.
//!
//! These are thin wrappers (newtypes) around primitives with custom
//! serialization, validation, or display logic.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ---------------------------------------------------------------------------
// BoolOrU32 serde helpers
// ---------------------------------------------------------------------------

/// Trait for types that serialize as `u32` (or `bool` for the sentinel value)
/// and deserialize from either a boolean or a non-negative integer.
///
/// `true` maps to `TRUE_VALUE` (typically `u32::MAX`), `false` maps to 0.
///
/// Implementors only need to specify `TRUE_VALUE`; the `From<u32>` and
/// `Into<u32>` conversions are provided via derive_more or manual impls.
pub trait BoolOrU32: Sized + Copy + From<u32> + Into<u32> {
    const TRUE_VALUE: u32;

    fn bool_or_u32_serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw: u32 = (*self).into();
        if raw == Self::TRUE_VALUE {
            serializer.serialize_bool(true)
        } else {
            serializer.serialize_u32(raw)
        }
    }

    fn bool_or_u32_deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        struct Visitor<T>(std::marker::PhantomData<T>);

        impl<T: BoolOrU32> serde::de::Visitor<'_> for Visitor<T> {
            type Value = T;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a boolean or non-negative integer")
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<T, E> {
                Ok(T::from(if v { T::TRUE_VALUE } else { 0 }))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<T, E> {
                Ok(T::from(u32::try_from(v).unwrap_or(T::TRUE_VALUE)))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<T, E> {
                if v < 0 {
                    Err(E::custom("value cannot be negative"))
                } else {
                    self.visit_u64(v as u64)
                }
            }
        }

        deserializer.deserialize_any(Visitor::<Self>(std::marker::PhantomData))
    }
}

// ---------------------------------------------------------------------------
// MemoryLimit
// ---------------------------------------------------------------------------

/// A byte-size type that accepts human-readable strings like "50MB", "1GiB", etc.
#[derive(Clone, Copy, PartialEq, Eq, humanbyte::HumanByte)]
pub struct MemoryLimit(pub u64);

impl JsonSchema for MemoryLimit {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("MemoryLimit")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Memory limit in human-readable format, e.g. '50MB', '1GiB', '512KB'"
        })
    }
}

// ---------------------------------------------------------------------------
// CpuLimit
// ---------------------------------------------------------------------------

/// CPU usage limit as a percentage (e.g. `80.0` = 80% of one core, `200.0` = 2 cores).
#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, derive_more::Into, derive_more::Display,
)]
#[display("{}%", _0)]
#[into(f64)]
#[serde(try_from = "f64")]
pub struct CpuLimit(pub f32);

impl TryFrom<f64> for CpuLimit {
    type Error = String;

    fn try_from(v: f64) -> std::result::Result<Self, Self::Error> {
        if v <= 0.0 {
            return Err("cpu_limit must be positive".into());
        }
        Ok(Self(v as f32))
    }
}

impl JsonSchema for CpuLimit {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("CpuLimit")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "number",
            "description": "CPU usage limit as a percentage (e.g. 80 for 80% of one core, 200 for 2 cores)",
            "exclusiveMinimum": 0
        })
    }
}

// ---------------------------------------------------------------------------
// StopSignal
// ---------------------------------------------------------------------------

/// Unix signal for graceful daemon shutdown (the first signal sent before SIGKILL).
///
/// Accepts signal names with or without `SIG` prefix, case-insensitive:
/// `"SIGINT"`, `"INT"`, `"sigint"` are all equivalent.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Deserialize, derive_more::Into, derive_more::Display,
)]
#[display("SIG{}", self.name())]
#[into(i32)]
#[serde(try_from = "String")]
pub struct StopSignal(i32);

const SIGNAL_TABLE: &[(&str, i32)] = &[
    ("HUP", libc::SIGHUP),
    ("INT", libc::SIGINT),
    ("QUIT", libc::SIGQUIT),
    ("TERM", libc::SIGTERM),
    ("USR1", libc::SIGUSR1),
    ("USR2", libc::SIGUSR2),
];

impl StopSignal {
    pub fn name(self) -> &'static str {
        SIGNAL_TABLE
            .iter()
            .find(|(_, sig)| *sig == self.0)
            .map(|(name, _)| *name)
            .unwrap_or("UNKNOWN")
    }
}

impl Default for StopSignal {
    fn default() -> Self {
        Self(libc::SIGTERM)
    }
}

impl TryFrom<String> for StopSignal {
    type Error = String;

    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        let upper = s.trim().to_ascii_uppercase();
        let name = upper.strip_prefix("SIG").unwrap_or(&upper);
        SIGNAL_TABLE
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, sig)| Self(*sig))
            .ok_or_else(|| format!("unsupported stop signal: {s}"))
    }
}

impl Serialize for StopSignal {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl JsonSchema for StopSignal {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("StopSignal")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Unix signal for graceful shutdown (e.g. 'SIGTERM', 'SIGINT', 'SIGHUP')",
            "enum": ["SIGTERM", "SIGINT", "SIGQUIT", "SIGHUP", "SIGUSR1", "SIGUSR2"]
        })
    }
}

// ---------------------------------------------------------------------------
// Retry
// ---------------------------------------------------------------------------

/// Retry configuration: `true` = indefinite, `false`/`0` = none, number = count.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema, derive_more::From, derive_more::Into,
)]
pub struct Retry(pub u32);

impl BoolOrU32 for Retry {
    const TRUE_VALUE: u32 = u32::MAX;
}

impl Retry {
    pub const INFINITE: Retry = Retry(u32::MAX);
    pub fn count(&self) -> u32 {
        self.0
    }
    pub fn is_infinite(&self) -> bool {
        self.0 == u32::MAX
    }
}

impl std::fmt::Display for Retry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_infinite() {
            f.write_str("infinite")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl Serialize for Retry {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.bool_or_u32_serialize(s)
    }
}

impl<'de> Deserialize<'de> for Retry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::bool_or_u32_deserialize(d)
    }
}
