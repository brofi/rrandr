use std::fmt::{self, Display};
use std::marker::PhantomData;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy)]
pub enum Auto<T> {
    Auto,
    Value(T),
}

impl<T> Auto<T> {
    pub fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        match self {
            Auto::Value(x) => x,
            Auto::Auto => f(),
        }
    }

    pub fn is_value_and(self, f: impl FnOnce(T) -> bool) -> bool {
        match self {
            Auto::Auto => false,
            Auto::Value(x) => f(x),
        }
    }
}

impl<T> Default for Auto<T> {
    fn default() -> Self { Self::Auto }
}

impl<T> From<T> for Auto<T> {
    fn from(val: T) -> Auto<T> { Auto::Value(val) }
}

impl<T: Display> fmt::Display for Auto<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Value(v) => v.fmt(f),
        }
    }
}

impl<T: Serialize> Serialize for Auto<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Auto::Auto => serializer.serialize_str("auto"),
            Auto::Value(v) => v.serialize(serializer),
        }
    }
}

macro_rules! impl_de_int {
    ($($type:ty),+) => {
        $(impl<'de> Deserialize<'de> for Auto<$type> {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_any(IntAutoVisitor(PhantomData))
            }
        })+
    };
}

impl_de_int!(u8, u16, u32, u64, i8, i16, i32, i64);

struct IntAutoVisitor<T>(PhantomData<T>);

impl<'de, T: TryFrom<i64>> Visitor<'de> for IntAutoVisitor<T> {
    type Value = Auto<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("`auto` or integer")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Auto<T>, E> {
        if value.to_lowercase() == "auto" {
            Ok(Auto::Auto)
        } else {
            Err(de::Error::invalid_value(Unexpected::Str(value), &self))
        }
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        v.try_into().map_or(
            Err(de::Error::custom("value too big or signed when it should be unsigned")),
            |i| Ok(Auto::Value(i)),
        )
    }
}

impl<'de> Deserialize<'de> for Auto<f64> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(FloatAutoVisitor)
    }
}

struct FloatAutoVisitor;

impl<'de> Visitor<'de> for FloatAutoVisitor {
    type Value = Auto<f64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("`auto` or floating point number")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Auto<f64>, E> {
        if value.to_lowercase() == "auto" {
            Ok(Auto::Auto)
        } else {
            Err(de::Error::invalid_value(Unexpected::Str(value), &self))
        }
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> { Ok(Auto::Value(v as f64)) }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> { Ok(Auto::Value(v)) }
}
