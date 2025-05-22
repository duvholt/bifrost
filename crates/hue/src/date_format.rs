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
            use serde::{self, Deserialize, de::Error};
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
            use serde::{self, Deserialize, de::Error};
            let s = String::deserialize(deserializer)?;
            let dt = chrono::NaiveDateTime::parse_from_str(&s, $fmt).map_err(Error::custom)?;
            Ok(dt)
        }
    };
}

macro_rules! date_deserializer_local_opt {
    ($type:ty, $fmt:expr) => {
        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<$type>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::{self, Deserialize, de::Error};
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
            use serde::{self, Deserialize, de::Error};
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
    use serde::{self, Deserialize, Deserializer, de::Error};

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

pub mod legacy_naive {
    use chrono::NaiveDateTime;

    date_serializer!(NaiveDateTime, super::FORMAT_LOCAL);
    date_deserializer_local!(NaiveDateTime, super::FORMAT_LOCAL);
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
    use std::fmt::Debug;

    use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
    use serde_json::de::StrRead;

    use crate::error::HueResult;

    fn de<T: Debug + Eq>(
        ds: &'static str,
        d1: &T,
        desi: impl Fn(&mut serde_json::Deserializer<StrRead>) -> serde_json::Result<T>,
    ) -> HueResult<()> {
        let mut deser = serde_json::Deserializer::from_str(ds);
        let d2 = desi(&mut deser)?;

        assert_eq!(*d1, d2);
        Ok(())
    }

    fn se(
        s1: &'static str,
        seri: impl Fn(&mut serde_json::Serializer<&mut Vec<u8>>) -> serde_json::Result<()>,
    ) -> HueResult<()> {
        let mut s2 = vec![];
        let mut ser = serde_json::Serializer::new(&mut s2);
        seri(&mut ser)?;

        eprintln!("{} vs {}", s1, s2.escape_ascii());
        assert_eq!(s1.as_bytes(), s2);
        Ok(())
    }

    fn date_utc() -> (&'static str, DateTime<Utc>) {
        let dt = Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap();
        ("\"2014-07-08T09:10:11Z\"", dt)
    }

    #[test]
    fn utc_de() -> HueResult<()> {
        let (ds, d1) = date_utc();
        de(ds, &d1, |de| super::utc::deserialize(de))
    }

    #[test]
    fn utc_se() -> HueResult<()> {
        let (s1, dt) = date_utc();
        se(s1, |ser| super::utc::serialize(&dt, ser))
    }

    fn date_utc_ms() -> (&'static str, DateTime<Utc>) {
        let dt = Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap();
        let dt = Utc
            .timestamp_millis_opt(dt.timestamp_millis() + 123)
            .unwrap();
        ("\"2014-07-08T09:10:11.123Z\"", dt)
    }

    #[test]
    fn utc_ms_de() -> HueResult<()> {
        let (ds, d1) = date_utc_ms();
        de(ds, &d1, |de| super::utc_ms::deserialize(de))
    }

    #[test]
    fn utc_ms_se() -> HueResult<()> {
        let (s1, dt) = date_utc_ms();
        se(s1, |ser| super::utc_ms::serialize(&dt, ser))
    }

    #[test]
    fn utc_ms_opt_de_some() -> HueResult<()> {
        let (ds, d1) = date_utc_ms();
        de(ds, &Some(d1), |de| super::utc_ms_opt::deserialize(de))
    }

    #[test]
    fn utc_ms_opt_de_none() -> HueResult<()> {
        de("null", &None, |de| super::utc_ms_opt::deserialize(de))
    }

    #[test]
    fn utc_ms_opt_se_some() -> HueResult<()> {
        let (s1, dt) = date_utc_ms();
        se(s1, |ser| super::utc_ms_opt::serialize(&Some(dt), ser))
    }

    #[test]
    fn utc_ms_opt_se_none() -> HueResult<()> {
        se("null", |ser| super::utc_ms_opt::serialize(&None, ser))
    }

    fn date_legacy_naive() -> (&'static str, NaiveDateTime) {
        let dt = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2014, 7, 8).unwrap(),
            NaiveTime::from_hms_opt(9, 10, 11).unwrap(),
        );
        ("\"2014-07-08T09:10:11\"", dt)
    }

    #[test]
    fn legacy_naive_de() -> HueResult<()> {
        let (ds, d1) = date_legacy_naive();
        de(ds, &d1, |de| super::legacy_naive::deserialize(de))
    }

    #[test]
    fn legacy_naive_se() -> HueResult<()> {
        let (s1, dt) = date_legacy_naive();
        se(s1, |ser| super::legacy_naive::serialize(&dt, ser))
    }

    fn date_legacy_local_opt() -> (&'static str, DateTime<Local>) {
        let dt = Local.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap();
        ("\"2014-07-08T09:10:11\"", dt)
    }

    #[test]
    fn legacy_local_opt_de_some() -> HueResult<()> {
        let (ds, d1) = date_legacy_local_opt();
        de(ds, &Some(d1), |de| super::legacy_local_opt::deserialize(de))
    }

    #[test]
    fn legacy_local_opt_se_some() -> HueResult<()> {
        let (s1, dt) = date_legacy_local_opt();
        se(s1, |ser| super::legacy_local_opt::serialize(&Some(dt), ser))
    }

    #[test]
    fn legacy_local_opt_de_none() -> HueResult<()> {
        de("null", &None, |de| super::legacy_local_opt::deserialize(de))
    }

    #[test]
    fn legacy_local_opt_se_none() -> HueResult<()> {
        se("null", |ser| super::legacy_local_opt::serialize(&None, ser))
    }

    #[test]
    fn update_utc_de() -> HueResult<()> {
        let (ds, d1) = date_utc();
        de(ds, &d1, |de| super::update_utc::deserialize(de))
    }
}
