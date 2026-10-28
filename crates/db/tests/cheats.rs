mod common;

use common::{mem_db, seed_rom};
use db::CheatsRepo;

#[tokio::test]
async fn add_list_update_delete_in_order() {
    let db = mem_db().await;
    seed_rom(&db, "rom-1", "snes").await;
    seed_rom(&db, "rom-2", "snes").await;
    let repo = CheatsRepo::new(db.clone());

    let a = repo.add("rom-1", "Vidas", "7E0DBF09", true).await.unwrap();
    let b = repo
        .add("rom-1", "Tempo", "C2A5-1D0F", false)
        .await
        .unwrap();
    repo.add("rom-2", "Outro jogo", "0000", true).await.unwrap();
    assert_eq!((a.position, b.position), (0, 1), "entram no fim da lista");

    let list = repo.list("rom-1").await.unwrap();
    assert_eq!(
        list.iter()
            .map(|c| c.description.as_str())
            .collect::<Vec<_>>(),
        ["Vidas", "Tempo"]
    );
    assert!(list[0].enabled && !list[1].enabled);

    let up = repo
        .update(b.id, "Tempo infinito", "C2A5-1D0F", true)
        .await
        .unwrap()
        .unwrap();
    assert!(up.enabled);
    assert_eq!(up.description, "Tempo infinito");
    assert!(repo.update(9999, "x", "y", false).await.unwrap().is_none());

    assert_eq!(repo.delete(a.id).await.unwrap().as_deref(), Some("rom-1"));
    assert_eq!(repo.list("rom-1").await.unwrap().len(), 1);
    assert_eq!(
        repo.list("rom-2").await.unwrap().len(),
        1,
        "o outro jogo não muda"
    );
}

#[tokio::test]
async fn deleting_the_rom_deletes_its_cheats() {
    let db = mem_db().await;
    seed_rom(&db, "rom-1", "snes").await;
    let repo = CheatsRepo::new(db.clone());
    repo.add("rom-1", "Vidas", "7E0DBF09", true).await.unwrap();
    sqlx::query("DELETE FROM roms WHERE id = 'rom-1'")
        .execute(&db)
        .await
        .unwrap();
    assert!(repo.list("rom-1").await.unwrap().is_empty());
}
