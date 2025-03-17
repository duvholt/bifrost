use std::collections::HashMap;

use clap::Parser;
use clap_stdin::FileOrStdin;
use json_diff_ng::compare_serde_values;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Deserializer, Value};

use bifrost::error::ApiResult;
use hue::api::ResourceRecord;
use hue::legacy_api::{
    ApiConfig, ApiGroup, ApiLight, ApiResourceLink, ApiRule, ApiScene, ApiSchedule, ApiSensor,
};

fn false_positive((a, b): &(&Value, &Value)) -> bool {
    a.is_number() && b.is_number() && a.as_f64() == b.as_f64()
}

#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Input {
    V1 {
        config: Value,
        groups: HashMap<String, Value>,
        lights: HashMap<String, Value>,
        resourcelinks: HashMap<String, Value>,
        rules: HashMap<String, Value>,
        scenes: HashMap<String, Value>,
        schedules: HashMap<String, Value>,
        sensors: HashMap<String, Value>,
    },
    V2 {
        errors: Vec<Value>,
        data: Vec<Value>,
    },
    V2Flat(Value),
}

fn compare(before: &Value, after: &Value, report: bool) -> ApiResult<bool> {
    let diffs = compare_serde_values(before, after, true, &[]).unwrap();
    let all_diffs = diffs.all_diffs();

    if !all_diffs
        .iter()
        .any(|x| x.1.values.map_or(true, |q| !false_positive(&q)))
    {
        return Ok(true);
    }

    /* in report mode, hide diff details */
    if report {
        return Ok(false);
    }

    log::error!("Difference detected");
    eprintln!("--------------------------------------------------------------------------------");
    println!("{}", serde_json::to_string(before)?);
    eprintln!("--------------------------------------------------------------------------------");
    println!("{}", serde_json::to_string(after)?);
    eprintln!("--------------------------------------------------------------------------------");
    for (d_type, d_path) in all_diffs {
        if let Some(ref ab) = d_path.values {
            if false_positive(ab) {
                continue;
            }
        }
        match d_type {
            json_diff_ng::DiffType::LeftExtra => {
                eprintln!(" - {d_path}");
            }
            json_diff_ng::DiffType::Mismatch => {
                eprintln!(" * {d_path}");
            }
            json_diff_ng::DiffType::RightExtra => {
                eprintln!(" + {d_path}");
            }
            json_diff_ng::DiffType::RootMismatch => {
                eprintln!("{d_type}: {d_path}");
            }
        }
    }
    eprintln!();

    Ok(false)
}

pub struct Normalizer<'a> {
    name: &'a str,
    items: usize,
    errors: usize,
    width: usize,
    index: usize,
    report: bool,
}

impl<'a> Normalizer<'a> {
    #[must_use]
    pub const fn new(name: &'a str, width: usize, report: bool) -> Self {
        Self {
            name,
            index: 0,
            items: 0,
            errors: 0,
            width,
            report,
        }
    }

    pub fn error(&mut self) {
        self.errors += 1;
    }

    pub fn parse<T>(&mut self, obj: Value) -> ApiResult<T>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        let data: Result<T, _> = serde_json::from_value(obj);
        Ok(data.inspect_err(|err| {
            self.errors += 1;
            log::error!(
                "{name:width$} | >> Parse error {err} (object index {index})",
                name = self.name,
                width = self.width,
                index = self.index
            );
            /* eprintln!("{}", &serde_json::to_string(&before)?); */
        })?)
    }

    fn roundtrip<T>(&mut self, item: &Value) -> ApiResult<()>
    where
        T: Serialize + DeserializeOwned + std::fmt::Debug,
    {
        let value = self.parse::<T>(item.clone())?;
        self.items += 1;
        self.index += 1;
        let after = serde_json::to_value(&value)?;

        if !compare(item, &after, self.report)? {
            self.errors += 1;
        }
        Ok(())
    }

    fn test<T>(&mut self, item: &Value)
    where
        T: Serialize + DeserializeOwned + std::fmt::Debug,
    {
        let _ = self.roundtrip::<T>(item);
    }

    pub fn summary(&self) {
        let errors = self.errors;
        let items = self.items;
        let name = self.name;
        let width = self.width;
        if errors > 0 {
            log::error!("{name:width$} | {items:5} items | {errors:5} errors |");
        } else {
            log::info!("{name:width$} | {items:5} items |           OK |");
        }
    }
}

fn process_file(file: FileOrStdin, width: usize, report: bool) -> ApiResult<()> {
    let name = if file.is_stdin() {
        "<stdin>"
    } else {
        &file.filename().to_string()
    };
    let stream = Deserializer::from_reader(file.into_reader().unwrap()).into_iter::<Value>();

    let mut nml = Normalizer::new(name, width, report);

    for obj in stream {
        let obj = obj?;

        let Ok(msg) = nml.parse::<Input>(obj) else {
            continue;
        };

        match msg {
            Input::V1 {
                config,
                groups,
                lights,
                resourcelinks,
                rules,
                scenes,
                schedules,
                sensors,
            } => {
                /* log::info!("v1 detected"); */
                nml.test::<ApiConfig>(&config);

                for item in groups.values() {
                    nml.test::<ApiGroup>(item);
                }
                for item in lights.values() {
                    nml.test::<ApiLight>(item);
                }
                for item in resourcelinks.values() {
                    nml.test::<ApiResourceLink>(item);
                }
                for item in rules.values() {
                    nml.test::<ApiRule>(item);
                }
                for item in scenes.values() {
                    nml.test::<ApiScene>(item);
                }
                for item in schedules.values() {
                    nml.test::<ApiSchedule>(item);
                }
                for item in sensors.values() {
                    nml.test::<ApiSensor>(item);
                }
            }
            Input::V2 { data, .. } => {
                /* log::info!("v2 detected"); */
                for item in data {
                    nml.test::<ResourceRecord>(&item);
                }
            }
            Input::V2Flat(item) => {
                /* log::info!("v2flat detected"); */
                nml.test::<ResourceRecord>(&item);
            }
        }
    }

    nml.summary();

    Ok(())
}

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    /// input files
    #[arg(name = "files")]
    files: Vec<FileOrStdin>,

    /// show only per-file summary
    #[arg(short, name = "report", default_value_t = false)]
    report: bool,
}

impl Args {
    pub fn longest_filename(&self) -> usize {
        self.files
            .iter()
            .map(|b| b.filename().len().max(5))
            .max()
            .unwrap_or(5)
    }
}

fn main() -> ApiResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let args = Args::parse();
    let width = args.longest_filename();

    for file in args.files {
        process_file(file, width, args.report)?;
    }

    Ok(())
}
