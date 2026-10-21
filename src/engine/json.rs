//! Conversion YAML → JSON et projection des propriétés de frontmatter.
//!
//! Le frontmatter est analysé par le vault (`vault::frontmatter`), ce module ne
//! fait que le convertir en JSON pour l'API.

use serde_json::{Map, Value};

/// Convertit une valeur YAML en valeur JSON.
///
/// Les clés de mapping non textuelles sont converties en chaînes (un entier
/// `1` devient la clé `"1"`), afin de produire un objet JSON valide.
pub fn yaml_to_json(value: &serde_yaml::Value) -> Value {
    match value {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(flag) => Value::Bool(*flag),
        serde_yaml::Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Value::from(value)
            } else if let Some(value) = number.as_u64() {
                Value::from(value)
            } else if let Some(value) = number.as_f64() {
                serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        serde_yaml::Value::String(text) => Value::String(text.clone()),
        serde_yaml::Value::Sequence(items) => {
            Value::Array(items.iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(mapping) => {
            let mut object = Map::new();
            for (key, item) in mapping {
                object.insert(yaml_key_to_string(key), yaml_to_json(item));
            }
            Value::Object(object)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}

/// Frontmatter complet converti en objet JSON.
///
/// Un frontmatter absent, ou dont la racine n'est pas un mapping, donne un
/// objet vide.
pub fn frontmatter_to_json(frontmatter: Option<&serde_yaml::Value>) -> Value {
    match frontmatter {
        Some(value @ serde_yaml::Value::Mapping(_)) => yaml_to_json(value),
        _ => Value::Object(Map::new()),
    }
}

/// Projection « propriétés » d'un frontmatter.
///
/// Seules les clés dont la valeur est un scalaire ou une séquence de scalaires
/// sont retenues (c'est ce qu'Obsidian affiche comme propriétés) ; une
/// séquence d'un seul élément est ramenée au scalaire correspondant.
pub fn properties_to_json(frontmatter: Option<&serde_yaml::Value>) -> Value {
    let mut properties = Map::new();
    let Some(serde_yaml::Value::Mapping(mapping)) = frontmatter else {
        return Value::Object(properties);
    };
    for (key, value) in mapping {
        let Some(property) = scalar_property(value) else {
            continue;
        };
        properties.insert(yaml_key_to_string(key), property);
    }
    Value::Object(properties)
}

/// Convertit une valeur YAML en propriété JSON si elle est « affichable ».
fn scalar_property(value: &serde_yaml::Value) -> Option<Value> {
    match value {
        serde_yaml::Value::Sequence(items) => {
            let mut scalars = Vec::with_capacity(items.len());
            for item in items {
                scalars.push(scalar_property(item)?);
            }
            match scalars.len() {
                0 => Some(Value::Array(Vec::new())),
                1 => Some(scalars.into_iter().next().unwrap_or(Value::Null)),
                _ => Some(Value::Array(scalars)),
            }
        }
        serde_yaml::Value::Mapping(_) => None,
        serde_yaml::Value::Tagged(tagged) => scalar_property(&tagged.value),
        other => Some(yaml_to_json(other)),
    }
}

/// Convertit une clé YAML en clé d'objet JSON (toujours une chaîne).
fn yaml_key_to_string(key: &serde_yaml::Value) -> String {
    match key {
        serde_yaml::Value::String(text) => text.clone(),
        serde_yaml::Value::Bool(flag) => flag.to_string(),
        serde_yaml::Value::Number(number) => number.to_string(),
        serde_yaml::Value::Null => "null".to_string(),
        other => format!("{other:?}"),
    }
}
