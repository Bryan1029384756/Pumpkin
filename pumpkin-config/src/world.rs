use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct WorldGenerationConfig {
    /// The type of world generator to use
    pub generator_type: GeneratorType,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum GeneratorType {
    Vanilla,
    Void,
}

impl Default for WorldGenerationConfig {
    fn default() -> Self {
        Self {
            generator_type: GeneratorType::Vanilla,
        }
    }
}
