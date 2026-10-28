//! Serde helpers and macros used by the generated models.

use serde::de::{self, Deserialize, Deserializer};

/// Defines a string enum that only accepts the listed values.
macro_rules! closed_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
        }

        impl $name {
            /// All values of this enum.
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            /// The wire value of this variant.
            pub fn as_str(&self) -> &str {
                match self {
                    $( $name::$variant => $value, )*
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::util::UnknownVariant;
            fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                match s {
                    $( $value => Ok($name::$variant), )*
                    _ => Err($crate::util::UnknownVariant(s.to_owned())),
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> ::std::result::Result<Self, D::Error> {
                let s = <::std::borrow::Cow<'de, str> as ::serde::Deserialize>::deserialize(deserializer)?;
                s.parse().map_err(|_| ::serde::de::Error::unknown_variant(&s, &[$($value),*]))
            }
        }
    };
}

/// Defines a string enum that keeps unrecognized values in an `Unknown`
/// variant, so new server-side values don't break deserialization.
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version of the SDK does not know about.
            Unknown(String),
        }

        impl $name {
            /// The wire value of this variant.
            pub fn as_str(&self) -> &str {
                match self {
                    $( $name::$variant => $value, )*
                    $name::Unknown(s) => s.as_str(),
                }
            }

            /// Whether this is a value this version of the SDK does not know about.
            pub fn is_unknown(&self) -> bool {
                matches!(self, $name::Unknown(_))
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = ::std::convert::Infallible;
            fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                Ok(match s {
                    $( $value => $name::$variant, )*
                    other => $name::Unknown(other.to_owned()),
                })
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                match s.parse() {
                    Ok(v) => v,
                    Err(never) => match never {},
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> ::std::result::Result<Self, D::Error> {
                let s = <::std::borrow::Cow<'de, str> as ::serde::Deserialize>::deserialize(deserializer)?;
                Ok($name::from(s.as_ref()))
            }
        }
    };
}

/// Defines a unit type that (de)serializes to exactly one JSON value.
macro_rules! literal {
    ($name:ident = $value:literal) => {
        #[doc = concat!("The literal JSON value `", stringify!($value), "`.")]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct $name;

        impl $name {
            /// The JSON value of this literal.
            pub fn value() -> ::serde_json::Value {
                ::serde_json::json!($value)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::std::result::Result<S::Ok, S::Error> {
                ::serde::Serialize::serialize(&$value, serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::std::result::Result<Self, D::Error> {
                let v = <::serde_json::Value as ::serde::Deserialize>::deserialize(deserializer)?;
                if v == $name::value() {
                    Ok($name)
                } else {
                    Err(::serde::de::Error::custom(format_args!(
                        "expected {}, found {}",
                        stringify!($value),
                        v
                    )))
                }
            }
        }

        impl $crate::util::Lenient for $name {
            fn from_value(v: ::serde_json::Value) -> ::std::result::Result<Self, String> {
                if v.is_null() || v == $name::value() {
                    Ok($name)
                } else {
                    Err(format!("expected {}, found {}", stringify!($value), v))
                }
            }
        }
    };
}

/// Error returned when parsing an unknown value into a closed enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVariant(pub String);

impl std::fmt::Display for UnknownVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown variant `{}`", self.0)
    }
}

impl std::error::Error for UnknownVariant {}

/// Types that can be read leniently from a response, mirroring the coercions
/// the JS SDK applies to required primitive fields: `null` becomes the zero
/// value and numbers/booleans may arrive as strings.
pub trait Lenient: Sized {
    fn from_value(v: serde_json::Value) -> Result<Self, String>;
}

impl Lenient for String {
    fn from_value(v: serde_json::Value) -> Result<Self, String> {
        Ok(match v {
            serde_json::Value::Null => String::new(),
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        })
    }
}

impl Lenient for f64 {
    fn from_value(v: serde_json::Value) -> Result<Self, String> {
        match v {
            serde_json::Value::Null => Ok(0.0),
            serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| format!("invalid number {n}")),
            serde_json::Value::String(s) => s
                .trim()
                .parse()
                .map_err(|_| format!("expected a number, found {s:?}")),
            other => Err(format!("expected a number, found {other}")),
        }
    }
}

impl Lenient for i64 {
    fn from_value(v: serde_json::Value) -> Result<Self, String> {
        match v {
            serde_json::Value::Null => Ok(0),
            serde_json::Value::Number(n) => n
                .as_i64()
                .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
                .ok_or_else(|| format!("expected an integer, found {n}")),
            serde_json::Value::String(s) => s
                .trim()
                .parse()
                .map_err(|_| format!("expected an integer, found {s:?}")),
            other => Err(format!("expected an integer, found {other}")),
        }
    }
}

impl Lenient for bool {
    fn from_value(v: serde_json::Value) -> Result<Self, String> {
        match v {
            serde_json::Value::Null => Ok(false),
            serde_json::Value::Bool(b) => Ok(b),
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("true") => Ok(true),
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("false") => Ok(false),
            other => Err(format!("expected a boolean, found {other}")),
        }
    }
}

/// `deserialize_with` helper for [`Lenient`] fields.
pub(crate) fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Lenient,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    T::from_value(v).map_err(de::Error::custom)
}

/// `deserialize_with` helper for optional, nullable request fields: a present
/// `null` becomes `Some(None)`, an absent field stays `None` (via `default`).
pub(crate) fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
