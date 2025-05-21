use godot::prelude::*;
use toml::value::{Date, Datetime, Offset, Time};
use toml::{Table, Value};

/// Contains the methods and properties to parse a toml file and work with it.
#[derive(GodotClass, Default)]
#[class(base=Resource, init)]
pub struct TOML {
    #[var]
    pub data: Variant,
    pub error_line: i64,
    pub error_message: String,
    pub parsed_text: GString,
}

#[godot_api]
impl TOML {
    #[func]
    pub fn get_error_line(&self) -> i64 {
        self.error_line
    }

    #[func]
    pub fn get_error_message(&self) -> GString {
        self.error_message.clone().into()
    }

    #[func]
    pub fn get_parsed_text(&self) -> GString {
        self.parsed_text.clone()
    }

    #[func]
    pub fn parse(&mut self, toml_text: GString, keep_text: bool) -> i64 {
        self.parsed_text = match keep_text {
            true => toml_text.clone(),
            false => GString::new(),
        };

        let value = match toml::from_str::<Value>(&toml_text.to_string()) {
            Ok(v) => v,
            Err(e) => {
                self.error_message = e.message().to_owned();

                if let Some(span) = e.span() {
                    self.error_line = toml_text.count("\n", ..span.start) as i64;
                }

                return self.get_error_line();
            }
        };

        self.data = deserialize_variant(&value);

        0
    }

    #[func]
    pub fn parse_string(toml_string: GString) -> Variant {
        let mut toml = TOML::default();
        let error_line = toml.parse(toml_string, false);

        if error_line != 0 {
            godot_error!(
                "Error parsing on line {}: {}",
                error_line,
                toml.get_error_message()
            );
            Variant::nil()
        } else {
            toml.data
        }
    }

    #[func]
    pub fn stringify(variant: Variant) -> GString {
        let intermediate = serialize_variant(variant);
        godot_print!("Intermediate {intermediate}");
        toml::to_string_pretty(&intermediate).unwrap().into()
    }
}

fn deserialize_variant(value: &Value) -> Variant {
    match value {
        Value::String(value) => value.to_variant(),
        Value::Integer(value) => value.to_variant(),
        Value::Float(value) => value.to_variant(),
        Value::Boolean(value) => value.to_variant(),
        Value::Datetime(value) => deserialize_datetime(value),
        Value::Array(value) => deserialize_array(value),
        Value::Table(value) => deserialize_dictionary(value),
    }
}

fn deserialize_array(vec: &Vec<Value>) -> Variant {
    let array: Array<Variant> = vec.iter().map(deserialize_variant).collect();
    array.to_variant()
}

fn deserialize_dictionary(table: &Table) -> Variant {
    let mut dictionary = Dictionary::new();

    for (key, value) in table.iter() {
        dictionary.set(key.clone(), deserialize_variant(value));
    }

    dictionary.to_variant()
}

fn deserialize_datetime(datetime: &Datetime) -> Variant {
    let mut dictionary = Dictionary::new();

    if let Some(date) = datetime.date {
        dictionary.set("day", date.day);
        dictionary.set("month", date.month);
        dictionary.set("year", date.year);
    }

    if let Some(time) = datetime.time {
        dictionary.set("hour", time.hour);
        dictionary.set("minute", time.minute);
        dictionary.set("second", time.second);
        dictionary.set("nanosecond", time.nanosecond);
    }

    if let Some(offset) = datetime.offset {
        dictionary.set(
            "offset_minute",
            match offset {
                Offset::Z => 0,
                Offset::Custom { minutes } => minutes,
            },
        );
    }

    dictionary.to_variant()
}

fn serialize_variant(variant: Variant) -> Value {
    match variant.get_type() {
        VariantType::STRING => Value::String(variant.try_to().unwrap()),
        VariantType::INT => Value::Integer(variant.try_to().unwrap()),
        VariantType::FLOAT => Value::Float(variant.try_to().unwrap()),
        VariantType::BOOL => Value::Boolean(variant.try_to().unwrap()),
        VariantType::ARRAY => serialize_array(variant.try_to().unwrap()),
        VariantType::DICTIONARY => serialize_dictionary(variant.try_to().unwrap()),
        _ => unimplemented!("{variant} is not yet serializable."),
    }
}

fn serialize_array(vec: Vec<Variant>) -> Value {
    let mut array = toml::value::Array::new();

    for variant in vec {
        array.push(serialize_variant(variant));
    }

    Value::Array(array)
}

fn serialize_dictionary(dict: Dictionary) -> Value {
    if let Some(datetime) = serialize_datetime(&dict) {
        return Value::Datetime(datetime);
    }

    let mut table = Table::new();

    for (key, value) in dict.iter_shared() {
        table.insert(key.to_string(), serialize_variant(value));
    }

    Value::Table(table)
}

const GODOT_DATETIME_KEYS: [&'static str; 10] = [
    "year",
    "month",
    "day",
    "weekday",
    "hour",
    "minute",
    "second",
    "nanosecond",
    "dst",
    "offset_minute",
];

macro_rules! get_int_or_default {
    ($dict:ident, $key:literal) => {
        match $dict.get($key) {
            None => 0,
            Some(value) => value.try_to().ok()?,
        }
    };
}

/// If the dictionary only contains [`GODOT_DATETIME_KEYS`], then it will be serialized as a TOML [`Datetime`].
fn serialize_datetime(dict: &Dictionary) -> Option<Datetime> {
    if !dict
        .keys_shared()
        .all(|key| GODOT_DATETIME_KEYS.contains(&key.to_string().as_str()))
    {
        return None;
    }

    // Todo date and time are still serialized even if none of their keys are present.
    let date = Date {
        year: get_int_or_default!(dict, "year"),
        month: get_int_or_default!(dict, "month"),
        day: get_int_or_default!(dict, "day"),
    };

    let time = Time {
        hour: get_int_or_default!(dict, "hour"),
        minute: get_int_or_default!(dict, "minute"),
        second: get_int_or_default!(dict, "second"),
        nanosecond: get_int_or_default!(dict, "nanosecond"),
    };

    let mut offset = None;

    if dict.contains_key("offset") {
        if let Some(minutes) = dict
            .get("offset_minute")
            .and_then(|v| Variant::try_to::<i16>(&v).ok())
        {
            offset = Some(match minutes {
                0 => Offset::Z,
                _ => Offset::Custom { minutes },
            });
        }
    }

    Some(Datetime {
        date: Some(date),
        time: Some(time),
        offset,
    })
}
