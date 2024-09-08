use crate::{
    config::{nyanpasu::ProxiesSelectorMode, Config},
    core::{
        clash::proxies::{Proxies, ProxiesGuard, ProxiesGuardExt},
        handle::Handle,
    },
};
use anyhow::Context;
use base64::{engine::general_purpose::STANDARD as base64_standard, Engine as _};
use indexmap::IndexMap;
use tauri::{menu::MenuBuilder, AppHandle, Manager, Runtime};
use tracing::{debug, error, warn};
use tracing_attributes::instrument;

#[instrument]
async fn loop_task() {
    loop {
        match ProxiesGuard::global().update().await {
            Ok(_) => {
                debug!("update proxies success");
            }
            Err(e) => {
                warn!("update proxies failed: {:?}", e);
            }
        }
        {
            let guard = ProxiesGuard::global().read();
            if guard.updated_at() == 0 {
                error!("proxies not updated yet!!!!");
                // TODO: add a error dialog or notification, and panic?
            }

            // else {
            //     let proxies = guard.inner();
            //     let str = simd_json::to_string_pretty(proxies).unwrap();
            //     debug!(target: "tray", "proxies info: {:?}", str);
            // }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await; // TODO: add a config to control the interval
    }
}

type GroupName = String;
type FromProxy = String;
type ToProxy = String;
type ProxySelectAction = (GroupName, FromProxy, ToProxy);
#[derive(PartialEq)]
enum TrayUpdateType {
    None,
    Full,
    Part(Vec<ProxySelectAction>),
}

struct TrayProxyItem {
    current: Option<String>,
    all: Vec<String>,
    r#type: String, // TODO: 转成枚举
}
type TrayProxies = IndexMap<String, TrayProxyItem>;

/// Convert raw proxies to tray proxies
fn to_tray_proxies(mode: &str, raw_proxies: &Proxies) -> TrayProxies {
    let mut tray_proxies = TrayProxies::new();
    if matches!(mode, "global" | "rule" | "script") {
        if mode == "global" || raw_proxies.proxies.is_empty() {
            let global = TrayProxyItem {
                current: raw_proxies.global.now.clone(),
                all: raw_proxies
                    .global
                    .all
                    .iter()
                    .map(|x| x.name.to_owned())
                    .collect(),
                r#type: "Selector".to_string(),
            };
            tray_proxies.insert("global".to_owned(), global);
        }
        for raw_group in raw_proxies.groups.iter() {
            let group = TrayProxyItem {
                current: raw_group.now.clone(),
                all: raw_group.all.iter().map(|x| x.name.to_owned()).collect(),
                r#type: raw_group.r#type.clone(),
            };
            tray_proxies.insert(raw_group.name.to_owned(), group);
        }
    }
    tray_proxies
}

fn diff_proxies(old_proxies: &TrayProxies, new_proxies: &TrayProxies) -> TrayUpdateType {
    // 1. check if the length of two map is different
    if old_proxies.len() != new_proxies.len() {
        return TrayUpdateType::Full;
    }
    // 2. check if the group matching
    let group_matching = new_proxies
        .keys()
        .cloned()
        .collect::<Vec<String>>()
        .iter()
        .zip(&old_proxies.keys().cloned().collect::<Vec<String>>())
        .filter(|&(new, old)| new == old)
        .count();
    if group_matching != old_proxies.len() {
        return TrayUpdateType::Full;
    }
    // 3. start checking the group content
    let mut actions = Vec::new();
    for (group, item) in new_proxies.iter() {
        let old_item = old_proxies.get(group).unwrap(); // safe to unwrap

        // check if the length of all list is different
        if item.all.len() != old_item.all.len() {
            return TrayUpdateType::Full;
        }

        // first diff the all list
        let all_matching = item
            .all
            .iter()
            .zip(&old_item.all)
            .filter(|&(new, old)| new == old)
            .count();
        if all_matching != old_item.all.len() {
            return TrayUpdateType::Full;
        }
        // then diff the current
        if item.current != old_item.current {
            actions.push((
                group.clone(),
                old_item.current.clone().unwrap(),
                item.current.clone().unwrap(),
            ));
        }
    }
    if actions.is_empty() {
        TrayUpdateType::None
    } else {
        TrayUpdateType::Part(actions)
    }
}

#[instrument]
pub async fn proxies_updated_receiver() {
    let (mut rx, mut tray_proxies_holder) = {
        let guard = ProxiesGuard::global().read();
        let proxies = guard.inner().to_owned();
        let mode = crate::utils::config::get_current_clash_mode();
        (
            guard.get_receiver(),
            to_tray_proxies(mode.as_str(), &proxies),
        )
    };

    loop {
        match rx.recv().await {
            Ok(_) => {
                debug!("proxies updated");
                if Handle::global().app_handle.lock().is_none() {
                    warn!("app handle not found");
                    continue;
                }
                Handle::mutate_proxies();
                {
                    let is_tray_selector_enabled = Config::verge()
                        .latest()
                        .clash_tray_selector
                        .unwrap_or_default()
                        != ProxiesSelectorMode::Hidden;
                    if !is_tray_selector_enabled {
                        continue;
                    }
                }
                // Do diff check
                let mode = crate::utils::config::get_current_clash_mode();
                let current_tray_proxies =
                    to_tray_proxies(mode.as_str(), ProxiesGuard::global().read().inner());

                match diff_proxies(&tray_proxies_holder, &current_tray_proxies) {
                    TrayUpdateType::Full => {
                        debug!("should do full update");
                        tray_proxies_holder = current_tray_proxies;
                        match Handle::update_systray() {
                            Ok(_) => {
                                debug!("update systray success");
                            }
                            Err(e) => {
                                warn!("update systray failed: {:?}", e);
                            }
                        }
                    }
                    TrayUpdateType::Part(action_list) => {
                        debug!("should do partial update, op list: {:?}", action_list);
                        tray_proxies_holder = current_tray_proxies;
                        platform_impl::update_selected_proxies(&action_list);
                        debug!("update selected proxies success");
                    }
                    _ => {}
                }
            }
            Err(e) => {
                warn!("proxies updated receiver failed: {:?}", e);
            }
        }
    }
}

pub fn setup_proxies() {
    tauri::async_runtime::spawn(loop_task());
    tauri::async_runtime::spawn(proxies_updated_receiver());
}

mod platform_impl {
    use std::sync::atomic::AtomicBool;

    use super::{ProxySelectAction, TrayProxyItem};
    use crate::{
        config::nyanpasu::ProxiesSelectorMode,
        core::{clash::proxies::ProxiesGuard, handle::Handle},
    };
    use base64::{engine::general_purpose::STANDARD as base64_standard, Engine as _};
    use rust_i18n::t;
    use tauri::{
        menu::{
            CheckMenuItemBuilder, IsMenuItem, MenuBuilder, MenuItemBuilder, MenuItemKind, Submenu,
            SubmenuBuilder,
        },
        AppHandle, Manager, Runtime,
    };
    use tracing::warn;

    pub fn generate_group_selector<R: Runtime>(
        app_handle: &AppHandle<R>,
        group_name: &str,
        group: &TrayProxyItem,
    ) -> anyhow::Result<Submenu<R>> {
        let mut group_menu = SubmenuBuilder::new(app_handle, group_name);
        for item in group.all.iter() {
            let mut sub_item_builder = CheckMenuItemBuilder::new(item.clone()).id(format!(
                "select_proxy_{}_{}",
                base64_standard.encode(group_name),
                base64_standard.encode(item)
            ));
            if let Some(now) = group.current.clone() {
                if now == item.as_str() {
                    #[cfg(target_os = "linux")]
                    {
                        sub_item_builder.title = super::super::utils::selected_title(item);
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        sub_item_builder = sub_item_builder.checked(true);
                    }
                }
            }

            if !matches!(group.r#type.as_str(), "Selector" | "Fallback") {
                sub_item_builder = sub_item_builder.enabled(false);
            }

            group_menu = group_menu.item(&sub_item_builder.build(app_handle)?);
        }
        Ok(group_menu.build()?)
    }

    pub fn generate_selectors<'m, R: Runtime, M: Manager<R>>(
        app_handle: &AppHandle<R>,
        proxies: &super::TrayProxies,
    ) -> anyhow::Result<Vec<MenuItemKind<R>>> {
        let mut items = Vec::new();
        if proxies.is_empty() {
            items.push(MenuItemKind::MenuItem(
                MenuItemBuilder::new("No Proxies")
                    .id("no_proxies")
                    .enabled(false)
                    .build(app_handle)?,
            ));
            return Ok(items);
        }
        for (group, item) in proxies.iter() {
            let group_menu = generate_group_selector(app_handle, group, item)?;
            items.push(MenuItemKind::Submenu(group_menu));
        }
        Ok(items)
    }

    pub fn setup_tray<'m, R: Runtime, M: Manager<R>>(
        app_handle: &AppHandle<R>,
        mut menu: MenuBuilder<'m, R, M>,
    ) -> anyhow::Result<MenuBuilder<'m, R, M>> {
        let selector_mode = crate::config::Config::verge()
            .latest()
            .clash_tray_selector
            .unwrap_or_default();
        menu = match selector_mode {
            ProxiesSelectorMode::Hidden => return Ok(menu),
            ProxiesSelectorMode::Normal => menu.separator(),
            ProxiesSelectorMode::Submenu => menu,
        };
        let proxies = ProxiesGuard::global().read().inner().to_owned();
        let mode = crate::utils::config::get_current_clash_mode();
        let tray_proxies = super::to_tray_proxies(mode.as_str(), &proxies);
        let items = generate_selectors::<R, M>(app_handle, &tray_proxies)?;
        match selector_mode {
            ProxiesSelectorMode::Normal => {
                for item in items {
                    menu = menu.item(&item);
                }
            }
            ProxiesSelectorMode::Submenu => {
                let mut submenu = SubmenuBuilder::new(app_handle, t!("tray.select_proxies"));
                for item in items {
                    submenu = submenu.item(&item);
                }
                menu = menu.item(&submenu.build()?);
            }
            _ => {}
        }
        Ok(menu)
    }

    static TRAY_ITEM_UPDATE_BARRIER: AtomicBool = AtomicBool::new(false);

    #[tracing_attributes::instrument]
    pub fn update_selected_proxies(actions: &[ProxySelectAction]) {
        if TRAY_ITEM_UPDATE_BARRIER.load(std::sync::atomic::Ordering::Acquire) {
            warn!("tray item update is in progress, skip this update");
            return;
        }
        let app_handle = Handle::global().app_handle.lock();
        let tray_state = app_handle
            .as_ref()
            .unwrap()
            .state::<crate::core::tray::TrayState<tauri::Wry>>();
        TRAY_ITEM_UPDATE_BARRIER.store(true, std::sync::atomic::Ordering::Release);
        let menu = tray_state.menu.lock();
        for action in actions {
            tracing::debug!("update selected proxies: {:?}", action);
            let from = format!(
                "select_proxy_{}_{}",
                base64_standard.encode(&action.0),
                base64_standard.encode(&action.1)
            );
            let to = format!(
                "select_proxy_{}_{}",
                base64_standard.encode(&action.0),
                base64_standard.encode(&action.2)
            );

            match menu.get(&from) {
                Some(item) => match item.kind() {
                    MenuItemKind::Check(item) => {
                        if item.is_checked().is_ok_and(|x| x) {
                            let _ = item.set_checked(false);
                        }
                    }
                    MenuItemKind::MenuItem(item) => {
                        let _ = item.set_text(action.1.clone());
                    }
                    _ => {
                        warn!("failed to deselect, item is not a check item: {}", from);
                    }
                },
                None => {
                    warn!("failed to deselect, item not found: {}", from);
                }
            }
            match menu.get(&to) {
                Some(item) => match item.kind() {
                    MenuItemKind::Check(item) => {
                        if item.is_checked().is_ok_and(|x| !x) {
                            let _ = item.set_checked(true);
                        }
                    }
                    MenuItemKind::MenuItem(item) => {
                        let _ = item.set_text(action.2.clone());
                    }
                    _ => {
                        warn!("failed to select, item is not a check item: {}", from);
                    }
                },
                None => {
                    warn!("failed to select, item not found: {}", to);
                }
            }
        }
        TRAY_ITEM_UPDATE_BARRIER.store(false, std::sync::atomic::Ordering::Release);
    }
}

pub trait SystemTrayMenuProxiesExt<R: Runtime> {
    fn setup_proxies(self, app_handle: &AppHandle<R>) -> anyhow::Result<Self>
    where
        Self: Sized;
}

impl<'m, R: Runtime, M: Manager<R>> SystemTrayMenuProxiesExt<R> for MenuBuilder<'m, R, M> {
    fn setup_proxies(self, app_handle: &AppHandle<R>) -> anyhow::Result<Self> {
        platform_impl::setup_tray(app_handle, self)
    }
}

#[instrument]
pub fn on_system_tray_event(event: &str) {
    if !event.starts_with("select_proxy_") {
        return; // bypass non-select event
    }
    let parts: Vec<&str> = event.split('_').collect();
    if parts.len() != 4 {
        return; // bypass invalid event
    }

    let wrapper = move || -> anyhow::Result<()> {
        let group = String::from_utf8(base64_standard.decode(parts[2])?)?;
        let name = String::from_utf8(base64_standard.decode(parts[3])?)?;
        tracing::debug!("received select proxy event: {} {}", group, name);
        tauri::async_runtime::block_on(async move {
            ProxiesGuard::global()
                .select_proxy(&group, &name)
                .await
                .with_context(|| format!("select proxy failed, {} {}, cause: ", group, name))?;

            debug!("select proxy success: {} {}", group, name);
            Ok::<(), anyhow::Error>(())
        })?;
        Ok(())
    };

    if let Err(e) = wrapper() {
        // TODO: add a error dialog or notification
        error!("on_system_tray_event failed: {:?}", e);
    }
}
