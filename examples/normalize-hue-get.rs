use std::io::Read;

use bifrost::hue::api::ResourceRecord;
use bifrost::{error::ApiResult, hue::legacy_api::ApiUserConfig};

use clap::Parser;
use clap_stdin::FileOrStdin;
use json_diff_ng::compare_serde_values;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{de::IoRead, Deserializer, StreamDeserializer, Value};

fn false_positive((a, b): &(&Value, &Value)) -> bool {
    a.is_number() && b.is_number() && a.as_f64() == b.as_f64()
}

/*
 * Handle both "raw" hue bridge dumps, and the "linedump" format that is
 * generally easier to work with.
 *
 * For each element in the input, if it is an object, look for a ".data" array,
 * and if found, iterate over that. Otherwise, assume the whole object is what
 * we are parsing.
 */
fn extract<'a, R: Read + 'a>(
    stream: StreamDeserializer<'a, IoRead<R>, Value>,
) -> impl Iterator<Item = (usize, Value)> + 'a {
    stream
        .flat_map(|val| {
            val.as_ref()
                .unwrap()
                .as_object()
                .and_then(|obj| obj.get("data"))
                .map(|data| data.as_array().unwrap().to_owned())
                .unwrap_or_else(|| vec![val.unwrap()])
        })
        .enumerate()
}

fn compare(before: &Value, after: &Value, report: bool) -> ApiResult<bool> {
    let diffs = compare_serde_values(before, after, true, &[]).unwrap();
    let all_diffs = diffs.all_diffs();

    if !all_diffs
        .iter()
        .any(|x| x.1.values.map(|q| !false_positive(&q)).unwrap_or(true))
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

fn process_file<T>(file: FileOrStdin, width: usize, report: bool) -> ApiResult<()>
where
    T: DeserializeOwned + Serialize + std::fmt::Debug,
{
    let name = if file.is_stdin() {
        "<stdin>"
    } else {
        &file.filename().to_string()
    };
    let stream = Deserializer::from_reader(file.into_reader().unwrap()).into_iter::<Value>();

    let mut items = 0;
    let mut errors = 0;

    for (index, obj) in extract(stream) {
        items += 1;
        let before = obj;
        let data: Result<T, _> = serde_json::from_value(before.clone());

        let Ok(msg) = data else {
            errors += 1;
            let err = data.unwrap_err();
            log::error!(
                "{name:width$} | >> Parse error {err} (object index {})",
                index
            );
            /* eprintln!("{}", &serde_json::to_string(&before)?); */
            continue;
        };

        let after = serde_json::to_value(&msg)?;

        if !compare(&before, &after, report)? {
            errors += 1;
        }
    }

    if errors > 0 {
        log::warn!("{name:width$} | {items:5} items | {errors:5} errors |");
    } else {
        log::info!("{name:width$} | {items:5} items |           OK |");
    }

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

    /// input is v1 api objects (default: v2)
    #[arg(short = '1', name = "v1", default_value_t = false)]
    v1: bool,
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
        if args.v1 {
            process_file::<ApiUserConfig>(file, width, args.report)?;
        } else {
            process_file::<ResourceRecord>(file, width, args.report)?;
        }
    }

    Ok(())
}
