//! Persistência de hotkeys de sistema e mapeamentos de controle
//! (`system_hotkeys` / `controller_mappings`), incluindo combinação
//! hold+press serializada como JSON.

mod common;

use common::mem_db;
use db::{ControllerMappingsRepo, DevicePortsRepo, SystemHotkeysRepo};
use domain::hotkeys::{HotkeyBinding, SystemAction, SystemHotkeyRepository};
use domain::input::{
    ControllerLayoutEntry, ControllerMapping, ControllerMappingRepository, DevicePortRepository,
    MappingSource, RawInputEvent, RetroPadButton,
};

fn key(sc: u32) -> RawInputEvent {
    RawInputEvent::Keyboard { scancode: sc }
}

#[tokio::test]
async fn system_hotkey_set_is_one_row_per_action() {
    let repo = SystemHotkeysRepo::new(mem_db().await);

    repo.set(&HotkeyBinding {
        action: SystemAction::ToggleMenuOverlay,
        trigger: vec![key(1)],
        device_guid: None,
    })
    .await
    .unwrap();
    // rebind da mesma ação -> substitui, não acumula
    repo.set(&HotkeyBinding {
        action: SystemAction::ToggleMenuOverlay,
        trigger: vec![key(2), key(3)],
        device_guid: None,
    })
    .await
    .unwrap();
    repo.set(&HotkeyBinding {
        action: SystemAction::QuickSave,
        trigger: vec![key(9)],
        device_guid: None,
    })
    .await
    .unwrap();

    let mut list = repo.list().await.unwrap();
    assert_eq!(list.len(), 2);
    list.sort_by_key(|b| b.action.as_wire());

    let toggle = list
        .iter()
        .find(|b| b.action == SystemAction::ToggleMenuOverlay)
        .unwrap();
    assert_eq!(toggle.trigger, vec![key(2), key(3)]);

    repo.delete(SystemAction::ToggleMenuOverlay).await.unwrap();
    assert_eq!(repo.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn system_hotkey_rejects_empty_trigger() {
    let repo = SystemHotkeysRepo::new(mem_db().await);
    let err = repo
        .set(&HotkeyBinding {
            action: SystemAction::QuickLoad,
            trigger: vec![],
            device_guid: None,
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn controller_mapping_upsert_roundtrips_combo() {
    let repo = ControllerMappingsRepo::new(mem_db().await);
    let guid = "03000000deadbeef0000000000000000";

    repo.upsert(&ControllerMapping {
        guid: guid.into(),
        display_name: "Meu Controle".into(),
        layout: vec![ControllerLayoutEntry {
            trigger: vec![RawInputEvent::GamepadButton {
                device_guid: guid.into(),
                index: 0,
            }],
            button: RetroPadButton::B,
        }],
        source: MappingSource::UserOverride,
    })
    .await
    .unwrap();

    // segunda gravação com o mesmo guid substitui (não viola PK)
    repo.upsert(&ControllerMapping {
        guid: guid.into(),
        display_name: "Renomeado".into(),
        layout: vec![
            ControllerLayoutEntry {
                trigger: vec![RawInputEvent::GamepadButton {
                    device_guid: guid.into(),
                    index: 0,
                }],
                button: RetroPadButton::B,
            },
            ControllerLayoutEntry {
                trigger: vec![
                    RawInputEvent::GamepadButton {
                        device_guid: guid.into(),
                        index: 10,
                    },
                    RawInputEvent::GamepadButton {
                        device_guid: guid.into(),
                        index: 11,
                    },
                ],
                button: RetroPadButton::Start,
            },
        ],
        source: MappingSource::UserOverride,
    })
    .await
    .unwrap();

    let got = repo.get(guid).await.unwrap().unwrap();
    assert_eq!(got.display_name, "Renomeado");
    assert_eq!(got.layout.len(), 2);
    assert_eq!(got.layout[1].button, RetroPadButton::Start);
    assert_eq!(got.layout[1].trigger.len(), 2);
    assert_eq!(got.source, MappingSource::UserOverride);

    assert_eq!(repo.list().await.unwrap().len(), 1);
    repo.delete(guid).await.unwrap();
    assert!(repo.get(guid).await.unwrap().is_none());
}

#[tokio::test]
async fn device_port_assignment_creates_mapping_row_for_fk() {
    let db = mem_db().await;
    let repo = DevicePortsRepo::new(db.clone());
    let guid = "03000000cafef00d0000000000000000";

    // guid ainda sem controller_mappings — o set precisa criar a linha (FK).
    repo.set(guid, 2).await.unwrap();
    assert_eq!(repo.list().await.unwrap(), vec![(guid.to_string(), 2)]);

    // reatribuir substitui
    repo.set(guid, 0).await.unwrap();
    assert_eq!(repo.list().await.unwrap(), vec![(guid.to_string(), 0)]);

    // a linha de mapeamento vazia sobreviveu
    assert!(ControllerMappingsRepo::new(db.clone())
        .get(guid)
        .await
        .unwrap()
        .is_some());

    repo.clear(guid).await.unwrap();
    assert!(repo.list().await.unwrap().is_empty());
}
