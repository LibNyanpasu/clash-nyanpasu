use anyhow::Error;
use async_trait::async_trait;
use serde_yaml::Mapping;
use std::collections::HashMap;

use super::js;
use crate::enhance::{ScriptType, ScriptWrapper};

type Logs = Vec<String>;
pub type ProcessOutput = (Mapping, Logs);

#[async_trait]
pub trait Runner: Send + Sync {
    fn try_new() -> Result<Self, Error>
    where
        Self: std::marker::Sized;
    #[allow(dead_code)]
    /// Process profiles by script file path
    async fn process(&self, mapping: Mapping, path: &str) -> Result<ProcessOutput, Error>;

    /// Honey replacement - use in memory code str to load module and exec it!
    /// It might not be implemented - due to some embeded engine is not support.
    async fn process_honey(&self, mapping: Mapping, script: &str) -> Result<ProcessOutput, Error> {
        tracing::debug!("mapping: {:?}\nscript:{}", mapping, script);
        unimplemented!()
    }
}

pub struct RunnerManager {
    runners: HashMap<ScriptType, Box<dyn Runner>>,
}

impl RunnerManager {
    pub fn new() -> Self {
        Self {
            runners: HashMap::new(),
        }
    }
    // If the script runner is not exist, it should be created.
    pub fn get_or_init_runner(&mut self, script_type: &ScriptType) -> anyhow::Result<&dyn Runner> {
        if !self.runners.contains_key(script_type) {
            let runner = match script_type {
                ScriptType::JavaScript => Box::new(js::JSRunner::try_new()?) as Box<dyn Runner>,
                ScriptType::Lua => unimplemented!("LuaRunner is not implemented yet"),
            };
            self.runners.insert(script_type.clone(), runner);
        }
        Ok(self.runners.get(script_type).unwrap().as_ref())
    }

    pub fn process_script(
        &mut self,
        script: ScriptWrapper,
        config: Mapping,
    ) -> anyhow::Result<ProcessOutput> {
        let runner = self.get_or_init_runner(&script.0)?;
        tauri::async_runtime::block_on(runner.process_honey(config, script.1.as_str()))
    }
}
