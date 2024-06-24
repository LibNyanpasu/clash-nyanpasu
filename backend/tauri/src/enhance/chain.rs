use crate::{
    config::{nyanpasu::ClashCore, profile::item_type::ProfileItemType, ProfileItem},
    utils::{dirs, help},
};
use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;
use std::fs;

#[derive(Debug, Clone)]
pub struct ChainItem {
    pub uid: String,
    pub data: ChainTypeWrapper,
}

#[derive(Debug, Clone)]
pub enum ChainTypeWrapper {
    Merge(Mapping),
    Script(ScriptWrapper),
}

impl ChainTypeWrapper {
    pub fn new_js(data: Data) -> Self {
        Self::Script(ScriptWrapper(ScriptType::JavaScript, data))
    }

    pub fn new_lua(data: Data) -> Self {
        Self::Script(ScriptWrapper(ScriptType::Lua, data))
    }

    pub fn new_merge(data: Mapping) -> Self {
        Self::Merge(data)
    }
}

impl TryFrom<&ProfileItem> for ChainTypeWrapper {
    type Error = anyhow::Error;

    fn try_from(item: &ProfileItem) -> Result<Self, Self::Error> {
        use anyhow::Context;
        let r#type = item.r#type.as_ref().context("type is required")?;
        let file = item.file.clone().context("file is required")?;
        let path = dirs::app_profiles_dir()
            .context("profiles dir not found")?
            .join(file);

        if !path.exists() {
            anyhow::bail!("file not found: {:?}", path);
        }

        match r#type {
            ProfileItemType::Script(ScriptType::JavaScript) => Ok(ChainTypeWrapper::Script(
                ScriptWrapper(ScriptType::JavaScript, fs::read_to_string(path)?),
            )),
            ProfileItemType::Script(ScriptType::Lua) => Ok(ChainTypeWrapper::Script(
                ScriptWrapper(ScriptType::Lua, fs::read_to_string(path)?),
            )),
            ProfileItemType::Merge => Ok(ChainTypeWrapper::Merge(help::read_merge_mapping(&path)?)),
            _ => anyhow::bail!("unsupported type: {:?}", r#type),
        }
    }
}

impl TryFrom<&ProfileItem> for ChainItem {
    type Error = anyhow::Error;

    fn try_from(item: &ProfileItem) -> Result<Self, Self::Error> {
        let uid = item.uid.clone().unwrap_or("".into());
        let data = ChainTypeWrapper::try_from(item)?;
        Ok(Self { uid, data })
    }
}

impl From<&ProfileItem> for Option<ChainItem> {
    fn from(item: &ProfileItem) -> Self {
        let uid = item.uid.clone().unwrap_or("".into());
        let data = ChainTypeWrapper::try_from(item);
        match data {
            Err(_) => None,
            Ok(data) => Some(ChainItem { uid, data }),
        }
    }
}

type Data = String;
#[derive(Debug, Clone)]
pub struct ScriptWrapper(pub ScriptType, pub Data);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChainType {
    #[serde(rename = "merge")]
    Merge,
    #[serde(rename = "script")]
    Script(ScriptType),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Eq, PartialEq, Hash)]
pub enum ScriptType {
    #[default]
    #[serde(rename = "javascript")]
    JavaScript,
    #[serde(rename = "lua")]
    Lua,
}

#[derive(Debug, Clone)]
pub enum ChainSupport {
    Clash,
    Mihomo,
    ClashRs,
    All,
}

impl ChainItem {
    /// 内建支持一些脚本
    pub fn builtin() -> Vec<(ChainSupport, ChainItem)> {
        // meta 的一些处理
        let meta_guard = ChainItem::to_script(
            "verge_meta_guard",
            ChainTypeWrapper::new_js(include_str!("./builtin/meta_guard.js").to_string()),
        );

        // meta 1.13.2 alpn string 转 数组
        let hy_alpn = ChainItem::to_script(
            "verge_hy_alpn",
            ChainTypeWrapper::new_js(include_str!("./builtin/meta_hy_alpn.js").to_string()),
        );

        vec![
            (ChainSupport::Mihomo, hy_alpn),
            (ChainSupport::Mihomo, meta_guard),
        ]
    }

    pub fn to_script<U: Into<String>, D: Into<ChainTypeWrapper>>(uid: U, data: D) -> Self {
        Self {
            uid: uid.into(),
            data: data.into(),
        }
    }
}

impl ChainSupport {
    pub fn is_support(&self, core: Option<&ClashCore>) -> bool {
        match core {
            Some(core) => matches!(
                (self, core),
                (ChainSupport::All, _)
                    | (ChainSupport::Clash, ClashCore::ClashPremium)
                    | (ChainSupport::ClashRs, ClashCore::ClashRs)
                    | (
                        ChainSupport::Mihomo,
                        ClashCore::Mihomo | ClashCore::MihomoAlpha
                    )
            ),
            None => true,
        }
    }
}
