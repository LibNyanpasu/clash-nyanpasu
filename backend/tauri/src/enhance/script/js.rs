use std::sync::Arc;

// use crate::utils::dirs;

use super::runner::{ProcessOutput, Runner};
use anyhow::Context;
use async_trait::async_trait;
use parking_lot::Mutex;
use rquickjs::{
    async_with,
    loader::{
        BuiltinResolver,
        // FileResolver,
        ScriptLoader,
    },
    AsyncContext, AsyncRuntime, Function, Module,
};
use serde_yaml::Mapping;
use tauri::async_runtime;
use tracing_attributes::instrument;
pub struct JSRunner(AsyncRuntime);

#[async_trait]
impl Runner for JSRunner {
    #[instrument]
    fn try_new() -> Result<JSRunner, anyhow::Error> {
        let js_runtime = AsyncRuntime::new().context("failed to create rquickjs runtime")?;
        // let ctx = AsyncContext::full(&js_runtime);
        // let app_path = dirs::app_profiles_dir().context("failed to get app profiles dir")?;
        // let app_path = relative_path::RelativePathBuf::from_path(app_path)?;
        let resolver = (
            BuiltinResolver::default(), // .with_module(path)
                                        // FileResolver::default().with_path(app_path),
        );
        let loader = ScriptLoader::default();
        let runtime: AsyncRuntime = async_runtime::block_on(async move {
            let runtime = js_runtime;
            runtime.set_loader(resolver, loader).await;
            runtime
        });
        Ok(JSRunner(runtime))
    }

    async fn process(&self, mapping: Mapping, path: &str) -> Result<ProcessOutput, anyhow::Error> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("failed to read the script file")?;
        self.process_honey(mapping, &content).await
    }

    async fn process_honey(
        &self,
        mapping: Mapping,
        script: &str,
    ) -> Result<ProcessOutput, anyhow::Error> {
        let ctx = AsyncContext::full(&self.0)
            .await
            .context("failed to get a context")?;
        let outputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        let outputs_clone = outputs.clone();
        let print = move |msg: String| {
            let mut outs = outputs_clone.lock();
            tracing::debug!("script log: {:?}", msg);
            outs.push(msg.clone())
        };
        let script = utils::wrap_script_if_not_esm(script);
        let config = simd_json::to_string_pretty(&mapping)?;
        let mut result = async_with!(ctx => |ctx| {
            let raw_ctx = ctx.clone();
            let run = || async move {
                let global = ctx.globals();
                global
                    .set(
                        "print",
                        Function::new(ctx.clone(), print)?.with_name("print")?,
                    )
                    .context("failed to set print fn")?;
                // if user script fn is main(config): config should convert to esm
                let user_module = format!("{script};
                let config = JSON.parse('{config}');
                export let _processed_config = await main(config);
                ");
                println!("user_module: {:?}", user_module);
                Module::declare(ctx.clone(), "user_script", user_module).context("fail to define the user_script module")?;
                let promises = Module::evaluate(ctx.clone(), "process_honey", "
                import { _processed_config } from \"user_script\";
                globalThis.final_result = JSON.stringify(_processed_config);
                ").context("fail to eval the process_honey module")?;
                promises
                    .into_future::<()>()
                    .await
                    .context("fail to eval the module")?;
                let final_result = ctx.globals()
                    .get::<_, rquickjs::String>("final_result")
                    .context("fail to get the final result")?
                    .to_string()
                    .context("fail to convert the final result to string")?;
                Ok::<String, anyhow::Error>(final_result)
            };
            let res = run().await;
            res.map_err(|e| {
                // println!("error: {:?}", e);
                // check whether the error inside is a QuickJS exception
                // TODO: maybe the chains should be Context -> RawException -> Error
                for cause in e.chain() {
                    if let Some(rquickjs::Error::Exception) = cause.downcast_ref::<rquickjs::Error>() {
                        let raw_exception = raw_ctx.catch();
                        return e.context(format!("QuickJS exception: {:?}", raw_exception))
                    }
                }
                e
            })
        }).await?;
        let buff = unsafe { result.as_bytes_mut() };
        let mapping = simd_json::from_slice::<Mapping>(buff)
            .context("failed to convert the result to mapping")?;
        let outs = outputs.lock();
        Ok((mapping, outs.to_vec()))
    }
}

mod utils {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^function\b[\s\S]*?\bmain\b").unwrap());
    pub fn wrap_script_if_not_esm(script: &str) -> String {
        let script = script.trim_matches(&[' ', '\n', '\t', '\r']);
        if !RE.is_match(script) {
            script.to_string()
        } else {
            format!("export default {}", script)
        }
    }
}

mod test {

    #[test]
    fn test_wrap_script_if_not_esm() {
        let script = r#"function main(config) {
            return config
        };"#;
        let script = super::utils::wrap_script_if_not_esm(script);
        assert_eq!(
            script,
            "export default function main(config) {\n            return config\n        };"
        );
    }

    #[test]
    fn test_process_honey() {
        use super::super::runner::Runner;
        use super::JSRunner;
        let runner = JSRunner::try_new().unwrap();
        let mapping = serde_yaml::from_str(
            r#"
        rules:
            - 111
            - 222
        tun:
            enable: false
        dns:
            enable: false
        "#,
        )
        .unwrap();
        let script = r#"
        export default async function main(config) {
            if (Array.isArray(config.rules)) {
                config.rules = [...config.rules, "add"];
            }
            print(JSON.stringify(config));
            config.proxies = ["111"];
            return config;
        }"#;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
            let (mapping, outs) = runner.process_honey(mapping, script).await.unwrap();
            assert_eq!(
                mapping["rules"],
                serde_yaml::Value::Sequence(vec![
                    serde_yaml::Value::String("111".to_string()),
                    serde_yaml::Value::String("222".to_string()),
                    serde_yaml::Value::String("add".to_string()),
                ])
            );
            assert_eq!(
                mapping["proxies"],
                serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("111".to_string()),])
            );
            assert_eq!(outs, vec!["{\"rules\":[\"111\",\"222\"],\"tun\":{\"enable\":false,\"dns\":{\"enable\":false}},\"proxies\":[\"111\"]}"]);
        }) ;
    }
}
