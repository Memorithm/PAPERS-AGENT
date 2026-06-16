use std::fs;
use std::path::Path;

pub fn load_config(path: Option<&str>) -> serde_yaml::Value {
    let default_config = r#"
max_rounds: 50
sampling_policy: greedy
n_candidates_per_round: 3
n_context_nodes: 5
n_cognition: 5
patience: 10
llm:
  provider: ollama
  model: gemma4:e2b
  base_url: "http://localhost:11434"
  timeout_secs: 300
  max_tokens: 2048
  temperature: 0.3
"#;

    let mut config: serde_yaml::Value =
        serde_yaml::from_str(default_config).unwrap_or_default();

    if let Some(p) = path {
        if Path::new(p).exists() {
            if let Ok(content) = fs::read_to_string(p) {
                if let Ok(user_config) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    deep_merge(&mut config, &user_config);
                }
            }
        }
    }

    config
}

fn deep_merge(base: &mut serde_yaml::Value, override_val: &serde_yaml::Value) {
    match (base, override_val) {
        (serde_yaml::Value::Mapping(base_map), serde_yaml::Value::Mapping(override_map)) => {
            for (key, value) in override_map {
                if let Some(existing) = base_map.get_mut(key) {
                    deep_merge(existing, value);
                } else {
                    base_map.insert(key.clone(), value.clone());
                }
            }
        }
        (base_val, override_val) => {
            *base_val = override_val.clone();
        }
    }
}
