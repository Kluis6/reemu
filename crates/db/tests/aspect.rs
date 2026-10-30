//! Cascata da proporção da imagem: jogo → plataforma → todos → core.

use db::AspectRepo;

#[tokio::test]
async fn cascade_and_clear() {
    let db = db::connect_in_memory().await.unwrap();
    let repo = AspectRepo::new(db);

    let a = repo.get(Some("r1"), "snes").await.unwrap();
    assert_eq!(a.resolved(), "core");

    repo.set("default", "", Some("4:3")).await.unwrap();
    assert_eq!(
        repo.get(Some("r1"), "snes").await.unwrap().resolved(),
        "4:3"
    );

    repo.set("system", "snes", Some("fill")).await.unwrap();
    assert_eq!(
        repo.get(Some("r1"), "snes").await.unwrap().resolved(),
        "fill"
    );
    // outra plataforma não herda
    assert_eq!(repo.get(Some("r2"), "nes").await.unwrap().resolved(), "4:3");

    repo.set("rom", "r1", Some("16:9")).await.unwrap();
    let a = repo.get(Some("r1"), "snes").await.unwrap();
    assert_eq!(a.resolved(), "16:9");
    assert_eq!(a.at_rom.as_deref(), Some("16:9"));
    assert_eq!(a.at_system.as_deref(), Some("fill"));
    // jogo fora da biblioteca: só plataforma e padrão
    assert_eq!(repo.get(None, "snes").await.unwrap().resolved(), "fill");

    // atualizar no mesmo escopo e limpar
    repo.set("rom", "r1", Some("core")).await.unwrap();
    assert_eq!(
        repo.get(Some("r1"), "snes").await.unwrap().resolved(),
        "core"
    );
    repo.set("rom", "r1", None).await.unwrap();
    assert_eq!(
        repo.get(Some("r1"), "snes").await.unwrap().resolved(),
        "fill"
    );
}
