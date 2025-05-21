use fancy_regex::Regex;
use godot::prelude::*;
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
        toml::to_string_pretty(&serialize_variant(variant))
            .unwrap()
            .into()
    }
}

fn deserialize_variant(value: &Value) -> Variant {
    match value {
        Value::String(value) => value.to_variant(),
        Value::Integer(value) => value.to_variant(),
        Value::Float(value) => value.to_variant(),
        Value::Boolean(value) => value.to_variant(),
        Value::Datetime(_) => unimplemented!(),
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
    let mut table = Table::new();

    for key in dict.keys_shared() {
        let variant = dict.get(key.clone()).unwrap();
        table.insert(key.to_string(), serialize_variant(variant));
    }

    Value::Table(table)
}

/// Populates a dictionary with the parsed values of the toml table provided.
///
/// If a value contains a string, it is further parsed by convert_godot_types which checks to see if the string is a Godot type and then performs
/// the necessary conversions on it and adds it to the dictionary.
///
/// # Arguments
///
/// `toml` - The parsed toml.
/// `dictionary` - The dictionary to populate.
/// `table` - The Table from the parsed toml.
fn populate_toml_dictionary(
    toml: &Value,
    dictionary: &mut Dictionary,
    table: &toml::map::Map<String, Value>,
) {
    for (key, value) in table {
        let field_type = value.type_str();
        match field_type {
            "table" => {
                let sub_dic = &mut Dictionary::new();
                let new_table = table[key]
                    .as_table()
                    .expect("Unable to cast value to table");
                populate_toml_dictionary(toml, sub_dic, new_table);
                dictionary.set(key.to_variant(), sub_dic.to_variant());
            }
            "array" => {
                let mut dictionary_arr = VariantArray::new();
                let toml_arr = table[key]
                    .as_array()
                    .expect("Unable to cast value to array");
                for i in toml_arr {
                    let sub_dic = &mut Dictionary::new();
                    populate_toml_dictionary(toml, sub_dic, i.as_table().unwrap());
                    dictionary_arr.push(&sub_dic.to_variant());
                }
                dictionary.set(key.to_variant(), dictionary_arr.to_variant())
            }
            "integer" => dictionary.set(
                key.to_variant(),
                value
                    .as_integer()
                    .expect("Unable to cast value to integer")
                    .to_variant(),
            ),
            "string" => {
                let value_as_str = value.as_str().expect("Unable to cast value to string");
                // A simple check to that we can avoid the cost of regex if we don't need to is to check if the string contains a parenthesis.
                if value_as_str.contains("(") {
                    encode_godot_types(dictionary, key, value_as_str);
                } else {
                    dictionary.set(key.to_variant(), value_as_str.to_variant())
                }
            }
            "float" => dictionary.set(
                key.to_variant(),
                value
                    .as_float()
                    .expect("Unable to cast value to float")
                    .to_variant(),
            ),
            "boolean" => dictionary.set(
                key.to_variant(),
                value
                    .as_bool()
                    .expect("Unable to cast value to bool")
                    .to_variant(),
            ),
            "datetime" => dictionary.set(
                key.to_variant(),
                value
                    .as_datetime()
                    .expect("Unable to cast value to float")
                    .to_string()
                    .to_variant(),
            ),
            _ => (),
        }
    }
}

/// Checks to see if a toml string is a Godot type and if so use `set_godot_type_to_dictionary`
///
/// # Arguments
///
/// `dictionary` - A reference to the dictionary so that the Godot types can be added to it.
/// `key` - The key of the current string item being checked.
/// `value` - The value of the current string item being checked.
fn encode_godot_types(dictionary: &mut Dictionary, key: &str, value: &str) {
    // Create a pattern to check for Godot types (Vector2, Rect2, etc.) and check to see if there are any matches in the string.
    // let type_re = Regex::new(r"((?:\/)?(\w+))").expect("Unable to create regex for type");
    let type_re =
        Regex::new(r"((?:\/)?([a-zA-Z0-9\.]+))").expect("Unable to create regex for type");
    let mut type_idx = 0;
    let mut type_results: Vec<&str> = vec![];
    while let Some(t) = type_re
        .captures_from_pos(value, type_idx)
        .expect("Unable to get captures")
    {
        type_results.push(t.get(1).expect("Unable to get capture group").as_str());
        type_idx = t.get(0).expect("Unable to get capture group").end();
    }

    let godot_types: [&str; 8] = [
        "Vector2",
        "Vector3",
        "Color",
        "Rect2",
        "Plane",
        "Transform2D",
        "Basis",
        "Transform",
    ];

    if !godot_types.contains(&type_results[0]) {
        dictionary.set(key.to_variant(), value.to_variant());
        return;
    }

    // If there is a pattern, then we need to decode the regex results into the Godot type.
    set_godot_type_to_dictionary(type_results, key, dictionary, &mut None, &mut None);
}

/// Returns a Vector2 at the specified (x, y) location.
///
/// # Arguments
///
/// `x` - The x position of the Vector2.
/// `y` - The y position of the Vector2.
fn encode_vector2(x: &str, y: &str) -> Vector2 {
    let vec_x: f32 = x.trim().parse().expect("Unable to cast to f32");
    let vec_y: f32 = y.trim().parse().expect("Unable to cast to f32");

    Vector2::new(vec_x, vec_y)
}

/// Returns a Vector3 with the specified x, y, and z values.
///
/// # Arguments
///
/// `x` - The x value of the Vector3.
/// `y` - The y value of the Vector3.
/// `z` - The z value of the Vector3.
fn encode_vector3(x: &str, y: &str, z: &str) -> Vector3 {
    let vec_x: f32 = x.trim().parse().expect("Unable to cast to f32");
    let vec_y: f32 = y.trim().parse().expect("Unable to cast to f32");
    let vec_z: f32 = z.trim().parse().expect("Unable to cast to f32");

    Vector3::new(vec_x, vec_y, vec_z)
}

/// Returns a Rect2 at the specified point and with the specified size.
///
/// # Arguments
///
/// `pos_vec` - The Vector2 that defines the Rect2's position.
/// `size_vec` - The Vector2 that defines the Rect's size.
fn encode_rect2(pos_vec: Vector2, size_vec: Vector2) -> Rect2 {
    Rect2::new(
        Vector2::new(pos_vec.x, pos_vec.y),
        Vector2::new(size_vec.x, size_vec.y),
    )
}

/// Returns a Transform with the provided axis vectors and the origin vector.
///
/// # Arguments
///
/// `x_axis_vec` - The x-axis vector for the transform.
/// `y_axis_vec` - The y-axis vector for the transform.
/// `z_axis_vec` - The z-axis vector for the transform.
/// `origin_vec` - The vector that specifies the origin of the transform.
fn encode_transform(
    x_axis_vec: Vector3,
    y_axis_vec: Vector3,
    z_axis_vec: Vector3,
    origin_vec: Vector3,
) -> Transform3D {
    Transform3D {
        basis: encode_basis(x_axis_vec, y_axis_vec, z_axis_vec),
        origin: origin_vec,
    }
}

/// Returns a Plane with the provided normal Vector and d value.
///
/// # Arguments
///
/// `normal_vec` - The Plane's normal vector.
/// `d` - The d value of the Plane.
fn encode_plane(normal_vec: Vector3, d: &str) -> Plane {
    let d_parsed: f32 = d.trim().parse().expect("Unable to cast to f32");

    Plane {
        normal: normal_vec,
        d: d_parsed,
    }
}

/// Returns a 3x3 matrix used consisting of Vector3 values for x, y, and z.
///
/// # Arguments
///
/// `x` - The Vector3 that defines the x column.
/// `y` - The Vector3 that defines the y column.
/// `z` - The Vector3 that defines the z column.
fn encode_basis(x: Vector3, y: Vector3, z: Vector3) -> Basis {
    Basis { rows: [x, y, z] }
}

/// Returns a Color from the specified rgb and optional alpha.
///
/// # Arguments
///
/// `r` - The red value of the color.
/// `g` - The green value of the color.
/// `b` - The blue value of the color.
/// `a` - The optional alpha value of the color.
fn encode_color(r: &str, g: &str, b: &str, a: Option<&str>) -> Color {
    let r_parsed: f32 = r.trim().parse().expect("Unable to cast to float");
    let g_parsed: f32 = g.trim().parse().expect("Unable to cast to float");
    let b_parsed: f32 = b.trim().parse().expect("Unable to cast to float");

    match a {
        Some(alpha) => {
            let a_parsed: f32 = alpha.trim().parse().expect("Unable to cast to float");
            Color::from_rgba(r_parsed, g_parsed, b_parsed, a_parsed)
        }
        None => Color::from_rgb(r_parsed, g_parsed, b_parsed),
    }
}

/// Takes the results of the regex provided by `convert_godot_types` and determines what Godot type needs to be created
/// and added to the dictionary.
///
/// # Arguments
///
/// `regex_results` - The vector of results from `convert_godot_types`.
/// `key` - The key of the current item.
/// `dictionary` - A reference to the dictionary so that Godot types can be added to it.
/// `vec2_pool` - An optional pool of Vector2s that is used when this function is called recursively for complex types made up of Vector2s.
/// `vec3_pool` - An optional pool of Vector3s that is used when this function is called recursively for complex types made up of Vector3s.
fn set_godot_type_to_dictionary(
    regex_results: Vec<&str>,
    key: &str,
    dictionary: &mut Dictionary,
    vec2_pool: &mut Option<&mut Vec<Vector2>>,
    vec3_pool: &mut Option<&mut Vec<Vector3>>,
) {
    for (i, item) in regex_results.iter().enumerate() {
        match item {
            &"Vector2" => {
                let vector2 = encode_vector2(regex_results[i + 1], regex_results[i + 2]);
                match vec2_pool {
                    Some(x) => x.push(vector2),
                    None => {
                        dictionary.set(key.to_variant(), vector2.to_variant());
                        break;
                    }
                }
            }
            &"Vector3" => {
                let vector3 = encode_vector3(
                    regex_results[i + 1],
                    regex_results[i + 2],
                    regex_results[i + 3],
                );
                match vec3_pool {
                    Some(x) => x.push(vector3),
                    None => {
                        dictionary.set(key.to_variant(), vector3.to_variant());
                        break;
                    }
                };
            }
            &"Color" => {
                let mut alpha: Option<&str> = None;
                if regex_results.len() == 5 {
                    alpha = Some(regex_results[i + 4]);
                }
                let color = encode_color(
                    regex_results[i + 1],
                    regex_results[i + 2],
                    regex_results[i + 3],
                    alpha,
                );
                dictionary.set(key.to_variant(), color.to_variant());
                break;
            }
            &"Rect2" => {
                // Since a Rect2 is a complex type that consists of Vector2's,
                // we need to run this function recursively to get
                // the Vector2 position and Vector2 size values.
                let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
                let vec2_pool: &mut Vec<Vector2> = &mut vec![];
                set_godot_type_to_dictionary(
                    new_regex_results,
                    key,
                    dictionary,
                    &mut Some(vec2_pool),
                    &mut None,
                );

                let rect2 = encode_rect2(vec2_pool[0], vec2_pool[1]);
                dictionary.set(key.to_variant(), rect2.to_variant());
                break;
            }
            &"Plane" => {
                // Plane is a complex type made up of a Vector3 and a float,
                // so we need to use recursion to get the Vector value.
                let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
                let vec3_pool: &mut Vec<Vector3> = &mut vec![];
                set_godot_type_to_dictionary(
                    new_regex_results,
                    key,
                    dictionary,
                    &mut None,
                    &mut Some(vec3_pool),
                );

                let plane = encode_plane(vec3_pool[0], regex_results[regex_results.len() - 1]);
                dictionary.set(key.to_variant(), plane.to_variant());
                break;
            }
            &"Transform2D" => {
                // Transform2D is a complex type made up of three Vector2s,
                // so we need to use recursion to get the Vector2 values.
                let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
                let vec2_pool: &mut Vec<Vector2> = &mut vec![];
                set_godot_type_to_dictionary(
                    new_regex_results,
                    key,
                    dictionary,
                    &mut Some(vec2_pool),
                    &mut None,
                );

                let transform2d = Transform2D::from_cols(vec2_pool[0], vec2_pool[1], vec2_pool[2]);
                dictionary.set(key.to_variant(), transform2d.to_variant());
                break;
            }
            &"Basis" => {
                // Basis is a complex type made up to three Vector3s,
                // so we need to use recursion to get the Vector3 values.
                let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
                let vec3_pool: &mut Vec<Vector3> = &mut vec![];
                set_godot_type_to_dictionary(
                    new_regex_results,
                    key,
                    dictionary,
                    &mut None,
                    &mut Some(vec3_pool),
                );

                let basis = encode_basis(vec3_pool[0], vec3_pool[1], vec3_pool[2]);
                dictionary.set(key.to_variant(), basis.to_variant());
                break;
            }
            &"Transform" => {
                // Transform is a complex type made up of four Vector3s,
                // so we need to use recursion to get the Vector3 values.
                let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
                let vec3_pool: &mut Vec<Vector3> = &mut vec![];
                set_godot_type_to_dictionary(
                    new_regex_results,
                    key,
                    dictionary,
                    &mut None,
                    &mut Some(vec3_pool),
                );

                let transform =
                    encode_transform(vec3_pool[0], vec3_pool[1], vec3_pool[2], vec3_pool[3]);
                dictionary.set(key.to_variant(), transform.to_variant());
                break;
            }
            _ => (),
        }
    }
}
