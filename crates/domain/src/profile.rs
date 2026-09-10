//! Perfil local do usuário — **um por instalação** (sem multi-perfil), igual
//! [`crate::audio::AudioConfigRepository`]. Definido no onboarding da 1ª
//! execução; futuramente ligado a uma rede social.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub bio: Option<String>,
    /// `"preset:1"`..`"preset:5"` (avatar embutido) ou `"file"` (imagem do
    /// usuário, em `<dados>/profile/avatar.<ext>`).
    pub avatar: String,
    /// `true` depois que o onboarding foi concluído.
    pub onboarded: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: String::new(),
            bio: None,
            avatar: "preset:1".into(),
            onboarded: false,
        }
    }
}

/// Persistência do perfil. Sempre uma linha única por instalação — daí
/// `get`/`update`, nunca `insert`/`list`.
#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn get(&self) -> Result<Profile, RepoError>;
    async fn update(&self, profile: &Profile) -> Result<(), RepoError>;
}
