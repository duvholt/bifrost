use std::{
    fs::File,
    io::{stdin, Read},
};

use bifrost::error::ApiResult;
use bifrost::hue::api::ResourceRecord;

use camino::Utf8PathBuf;
use clap::Parser;
use json_diff_ng::compare_serde_values;
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

fn compare(before: &Value, after: &Value, msg: ResourceRecord) -> ApiResult<bool> {
    let diffs = compare_serde_values(before, after, true, &[]).unwrap();
    let all_diffs = diffs.all_diffs();

    if !all_diffs
        .iter()
        .any(|x| x.1.values.map(|q| !false_positive(&q)).unwrap_or(true))
    {
        return Ok(true);
    }

    log::error!("Difference detected on {:?}", msg.obj.rtype());
    eprintln!("--------------------------------------------------------------------------------");
    println!("{}", serde_json::to_string(before)?);
    eprintln!("--------------------------------------------------------------------------------");
    println!("{}", serde_json::to_string(&msg)?);
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

fn process_file(reader: impl Read, name: &str, width: usize) -> ApiResult<()> {
    let stream = Deserializer::from_reader(reader).into_iter::<Value>();

    let mut items = 0;
    let mut errors = 0;

    for (index, obj) in extract(stream) {
        items += 1;
        let before = obj;
        let data: Result<ResourceRecord, _> = serde_json::from_value(before.clone());

        let Ok(msg) = data else {
            errors += 1;
            let err = data.unwrap_err();
            log::error!("Parse error {err:?} (object index {})", index);
            eprintln!("{}", &serde_json::to_string(&before)?);
            continue;
        };

        let after = serde_json::to_value(&msg)?;

        if !compare(&before, &after, msg)? {
            errors += 1;
        }
    }

    if errors > 0 {
        log::warn!("{name:width$} | {items:5} items | {errors:5} errors");
    } else {
        log::info!("{name:width$} | {items:5} items OK");
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    /// input files
    #[arg(name = "files", default_values_t = [Utf8PathBuf::from("-")])]
    files: Vec<Utf8PathBuf>,
}

fn main() -> ApiResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let args = Args::parse();
    let width = args
        .files
        .iter()
        .map(|b| b.as_str().len().max(5))
        .max()
        .unwrap();

    for file in args.files {
        if file == "-" {
            process_file(stdin(), "stdin", width)?;
        } else {
            process_file(File::open(&file)?, file.as_ref(), width)?;
        }
    }

    Ok(())
}
