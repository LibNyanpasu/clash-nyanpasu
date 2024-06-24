mod js;
mod lua;
pub mod runner;
pub use runner::RunnerManager;
// TODO: add test
// pub fn use_script(
//     script: ScriptWrapper,
//     config: Mapping,
// ) -> Result<(Mapping, Vec<(String, String)>)> {
//     match script.0 {
//         ScriptType::JavaScript => {

//         },
//         _ => unimplemented!("unsupported script type"),
//     }
// }

// #[test]
// fn test_script() {
//     let script = r#"
//     function main(config) {
//       if (Array.isArray(config.rules)) {
//         config.rules = [...config.rules, "add"];
//       }
//       console.log(config);
//       config.proxies = ["111"];
//       return config;
//     }
//   "#;

//     let config = r#"
//     rules:
//       - 111
//       - 222
//     tun:
//       enable: false
//     dns:
//       enable: false
//   "#;

//     let config = serde_yaml::from_str(config).unwrap();
//     let (config, results) = process_js(script.into(), config).unwrap();

//     let config_str = serde_yaml::to_string(&config).unwrap();

//     println!("{config_str}");

//     dbg!(results);
// }
