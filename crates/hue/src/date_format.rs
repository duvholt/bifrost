const FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
const FORMAT_MS: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";
const FORMAT_LOCAL: &str = "%Y-%m-%dT%H:%M:%S";
const UPDATE_FORMAT: &str = "%+";

macro_rules! date_serializer {
    ($type:ty, $fmt:expr) => {
        pub fn serialize<S>(date: &$type, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s = format!("{}", date.format($fmt));
            serializer.serialize_str(&s)
        }
    };
}

macro_rules! date_serializer_opt {
    ($type:ty, $fmt:expr) => {
        pub fn serialize<S>(date: &Option<$type>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            match date {
                Some(d) => serializer.serialize_str(&format!("{}", d.format($fmt))),
                None => serializer.serialize_none(),
            }
        }
    };
}

macro_rules! date_deserializer_utc {
    ($type:ty, $fmt:expr) => {
        pub fn deserialize<'de, D>(deserializer: D) -> Result<$type, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::{self, de::Error, Deserialize};
            let s = String::deserialize(deserializer)?;
            let dt = chrono::NaiveDateTime::parse_from_str(&s, $fmt).map_err(Error::custom)?;
            Ok(<$type>::from_naive_utc_and_offset(dt, Utc))
        }
    };
}

macro_rules! date_deserializer_local {
    ($type:ty, $fmt:expr) => {
        pub fn deserialize<'de, D>(deserializer: D) -> Result<$type, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::{self, de::Error, Deserialize};
            let s = String::deserialize(deserializer)?;
            let dt = chrono::NaiveDateTime::parse_from_str(&s, $fmt).map_err(Error::custom)?;
            dt.and_local_timezone(Local)
                .single()
                .ok_or_else(|| Error::custom("Localtime conversion failed"))
        }
    };
}

macro_rules! date_deserializer_local_opt {
    ($type:ty, $fmt:expr) => {
        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<$type>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::{self, de::Error, Deserialize};
            let Some(s) = Option::<String>::deserialize(deserializer)? else {
                return Ok(None);
            };

            Ok(Some(
                chrono::NaiveDateTime::parse_from_str(&s, super::FORMAT_LOCAL)
                    .map_err(Error::custom)?
                    .and_local_timezone(Local)
                    .single()
                    .ok_or_else(|| Error::custom("Localtime conversion failed"))?,
            ))
        }
    };
}

macro_rules! date_deserializer_utc_opt {
    ($type:ty, $fmt:expr) => {
        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<$type>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::{self, de::Error, Deserialize};
            let Some(s) = Option::<String>::deserialize(deserializer)? else {
                return Ok(None);
            };
            let dt = chrono::NaiveDateTime::parse_from_str(&s, $fmt).map_err(Error::custom)?;
            Ok(Some(<$type>::from_naive_utc_and_offset(dt, Utc)))
        }
    };
}

pub mod utc_ms {
    use chrono::{DateTime, Utc};

    date_serializer!(DateTime<Utc>, super::FORMAT_MS);
    date_deserializer_utc!(DateTime<Utc>, super::FORMAT_MS);
}

pub mod update_utc {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, de::Error, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = NaiveDateTime::parse_from_str(&s, super::UPDATE_FORMAT).map_err(Error::custom)?;
        Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
    }
}

pub mod utc {
    use chrono::{DateTime, Utc};

    date_serializer!(DateTime<Utc>, super::FORMAT);
    date_deserializer_utc!(DateTime<Utc>, super::FORMAT);
}

pub mod utc_ms_opt {
    use chrono::{DateTime, Utc};

    date_serializer_opt!(DateTime<Utc>, super::FORMAT_MS);
    date_deserializer_utc_opt!(DateTime<Utc>, super::FORMAT_MS);
}

pub mod legacy_local {
    use chrono::{DateTime, Local};

    date_serializer!(DateTime<Local>, super::FORMAT_LOCAL);
    date_deserializer_local!(DateTime<Local>, super::FORMAT_LOCAL);
}

pub mod legacy_local_opt {
    use chrono::{DateTime, Local};

    date_serializer_opt!(DateTime<Local>, super::FORMAT_LOCAL);
    date_deserializer_local_opt!(DateTime<Local>, super::FORMAT_LOCAL);
}

pub mod legacy_utc {
    use chrono::{DateTime, Utc};

    date_serializer!(DateTime<Utc>, super::FORMAT_LOCAL);
    date_deserializer_utc!(DateTime<Utc>, super::FORMAT_LOCAL);
}

pub mod legacy_utc_opt {
    use chrono::{DateTime, Utc};

    date_serializer_opt!(DateTime<Utc>, super::FORMAT_LOCAL);
    date_deserializer_utc_opt!(DateTime<Utc>, super::FORMAT_LOCAL);
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};

    use crate::error::ApiResult;

    fn date() -> (&'static str, DateTime<Utc>) {
        let dt = Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap();
        ("\"2014-07-08T09:10:11Z\"", dt)
    }

    #[test]
    fn utc_de() -> ApiResult<()> {
        let (ds, d1) = date();

        let mut deser = serde_json::Deserializer::from_str(ds);
        let d2 = super::utc::deserialize(&mut deser)?;

        assert_eq!(d1, d2);
        Ok(())
    }

    #[test]
    fn utc_se() -> ApiResult<()> {
        let (s1, dt) = date();

        let mut s2 = vec![];
        let mut ser = serde_json::Serializer::new(&mut s2);
        super::utc::serialize(&dt, &mut ser)?;

        assert_eq!(s1.as_bytes(), s2);
        Ok(())
    }
}
