mod constants;
mod ds;
mod process;

use std::fs::File;
use std::io::BufRead;
use std::io::{self, prelude::*, BufReader};

use chrono::DateTime;

use colored::*;
use constants::Message;
use ds::{key_node::KeyNode, mismatch::Mismatch};
use serde_json::{self, Value};
use std::{fmt, process as proc, str::FromStr};
use structopt::StructOpt;

const HELP: &str = r#"
Example:
json_diff f source1.json source2.json
json_diff d '{...}' '{...}'

Option:
f   :   read input from json files
d   :   read input from command line"#;

#[derive(Debug)]
struct AppError {
    message: Message,
}
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

enum InputReadMode {
    D,
    F,
}
impl FromStr for InputReadMode {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "d" => Ok(InputReadMode::D),
            "f" => Ok(InputReadMode::F),
            _ => Err(Self::Err {
                message: Message::BadOption,
            }),
        }
    }
}

#[derive(StructOpt)]
#[structopt(about = HELP)]
struct Cli {
    read_mode: InputReadMode,
    source1: String,
    source2: String,
}

fn error_exit(message: constants::Message) -> ! {
    eprintln!("{}", message);
    proc::exit(1);
}

fn main() {
    let args = Cli::from_args();

    let file1 = File::open(args.source1).unwrap();
    let mut r1 = BufReader::new(file1);
    let file2 = File::open(args.source2).unwrap();
    let mut r2 = BufReader::new(file2);
    let mut buffera = String::new();
    let mut bufferb = String::new();

    loop {
        if buffera.is_empty() {
            r1.read_line(&mut buffera).unwrap();
        }
        if bufferb.is_empty() {
            r2.read_line(&mut bufferb).unwrap();
        }

        if buffera.is_empty() || bufferb.is_empty() {
            break;
        }
        let compare_result = compare_jsons(&buffera, &bufferb);
        match compare_result {
            Mismatch {
                date_differ: Some(date),
                ..
            } => {
                if date {
                    print!("===\n{} : {}", "Missing event".red().bold(), &buffera);
                    buffera.clear();
                } else if !date {
                    print!("===\n{} : {}", "New event".red().bold(), &bufferb);
                    bufferb.clear();
                }
            }
            _ => {
                let no_mismatch = Mismatch {
                    left_only_keys: KeyNode::Nil,
                    right_only_keys: KeyNode::Nil,
                    keys_in_both: KeyNode::Nil,
                    date_differ: None,
                };
                if compare_result != no_mismatch {
                    println!("===\n{}", "Event modified :".red().bold());
                    print!("{} {}", "Before :".blue(), &buffera);
                    print!("{} {}", "After :".blue(), &bufferb);
                    display_output(compare_result);
                }
                buffera.clear();
                bufferb.clear();
            }
        }
    }
}

fn display_output(result: Mismatch) {
    let no_mismatch = Mismatch {
        left_only_keys: KeyNode::Nil,
        right_only_keys: KeyNode::Nil,
        keys_in_both: KeyNode::Nil,
        date_differ: None,
    };

    let stdout = io::stdout();
    let mut handle = io::BufWriter::new(stdout.lock());
    if no_mismatch == result {
        //writeln!(handle, "\n{}", Message::NoMismatch).unwrap();
    } else {
        match result.keys_in_both {
            KeyNode::Node(_) => {
                let mut keys = Vec::new();
                result.keys_in_both.absolute_keys(&mut keys, None);
                writeln!(handle, "\n{}:", Message::Mismatch).unwrap();
                for key in keys {
                    writeln!(handle, "{}", key).unwrap();
                }
            }
            KeyNode::Value(_, _) => writeln!(handle, "{}", Message::RootMismatch).unwrap(),
            KeyNode::Nil => (),
        }
        match result.left_only_keys {
            KeyNode::Node(_) => {
                let mut keys = Vec::new();
                result.left_only_keys.absolute_keys(&mut keys, None);
                writeln!(handle, "\n{}:", Message::LeftExtra).unwrap();
                for key in keys {
                    writeln!(handle, "{}", key.red().bold()).unwrap();
                }
            }
            KeyNode::Value(_, _) => error_exit(Message::UnknownError),
            KeyNode::Nil => (),
        }
        match result.right_only_keys {
            KeyNode::Node(_) => {
                let mut keys = Vec::new();
                result.right_only_keys.absolute_keys(&mut keys, None);
                writeln!(handle, "\n{}:", Message::RightExtra).unwrap();
                for key in keys {
                    writeln!(handle, "{}", key.green().bold()).unwrap();
                }
            }
            KeyNode::Value(_, _) => error_exit(Message::UnknownError),
            KeyNode::Nil => (),
        }
    }
}

fn compare_jsons(a: &str, b: &str) -> Mismatch {
    if let Ok(value1) = serde_json::from_str::<Value>(a) {
        if let Ok(value2) = serde_json::from_str::<Value>(b) {
            let d1 = DateTime::parse_from_rfc3339(value1["timestamp"].as_str().expect("KO"))
                .expect("KO");
            let d2 = DateTime::parse_from_rfc3339(value2["timestamp"].as_str().expect("KO"))
                .expect("KO");
            if d1 == d2 {
                process::match_json(&value1, &value2)
            } else {
                Mismatch {
                    left_only_keys: KeyNode::Nil,
                    right_only_keys: KeyNode::Nil,
                    keys_in_both: KeyNode::Nil,
                    date_differ: Some(d1 < d2),
                }
            }
        } else {
            error_exit(Message::JSON2);
        }
    } else {
        error_exit(Message::JSON1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;
    use serde_json::json;

    #[test]
    fn nested_diff() {
        let data1 = r#"{
            "a":"b", 
            "b":{
                "c":{
                    "d":true,
                    "e":5,
                    "f":9,
                    "h":{
                        "i":true,
                        "j":false
                    }
                }
            }
        }"#;
        let data2 = r#"{
            "a":"b",
            "b":{
                "c":{
                    "d":true,
                    "e":6,
                    "g":0,
                    "h":{
                        "i":false,
                        "k":false
                    }
                }
            }
        }"#;

        let expected_left = KeyNode::Node(hashmap! {
        "b".to_string() => KeyNode::Node(hashmap! {
                "c".to_string() => KeyNode::Node(hashmap! {
                        "f".to_string() => KeyNode::Nil,
                        "h".to_string() => KeyNode::Node( hashmap! {
                                "j".to_string() => KeyNode::Nil,
                            }
                        ),
                }
                ),
            }),
        });
        let expected_right = KeyNode::Node(hashmap! {
            "b".to_string() => KeyNode::Node(hashmap! {
                    "c".to_string() => KeyNode::Node(hashmap! {
                            "g".to_string() => KeyNode::Nil,
                            "h".to_string() => KeyNode::Node(hashmap! {
                                    "k".to_string() => KeyNode::Nil,
                                }
                            )
                        }
                    )
                }
            )
        });
        let expected_uneq = KeyNode::Node(hashmap! {
            "b".to_string() => KeyNode::Node(hashmap! {
                    "c".to_string() => KeyNode::Node(hashmap! {
                            "e".to_string() => KeyNode::Value(json!(5), json!(6)),
                            "h".to_string() => KeyNode::Node(hashmap! {
                                    "i".to_string() => KeyNode::Value(json!(true), json!(false)),
                                }
                            )
                        }
                    )
                }
            )
        });
        let expected = Mismatch::new(expected_left, expected_right, expected_uneq, None);

        let mismatch = compare_jsons(data1, data2);
        assert_eq!(mismatch, expected, "Diff was incorrect.");
    }

    #[test]
    fn no_diff() {
        let data1 = r#"{
            "a":"b", 
            "b":{
                "c":{
                    "d":true,
                    "e":5,
                    "f":9,
                    "h":{
                        "i":true,
                        "j":false
                    }
                }
            }
        }"#;
        let data2 = r#"{
            "a":"b", 
            "b":{
                "c":{
                    "d":true,
                    "e":5,
                    "f":9,
                    "h":{
                        "i":true,
                        "j":false
                    }
                }
            }
        }"#;

        assert_eq!(
            compare_jsons(data1, data2),
            Mismatch::new(KeyNode::Nil, KeyNode::Nil, KeyNode::Nil, None)
        );
    }

    #[test]
    fn no_json() {
        let data1 = r#"{}"#;
        let data2 = r#"{}"#;

        assert_eq!(
            compare_jsons(data1, data2),
            Mismatch::new(KeyNode::Nil, KeyNode::Nil, KeyNode::Nil, None)
        );
    }
}
