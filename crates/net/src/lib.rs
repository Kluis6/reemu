//! Rede dos downloads de servidores de terceiros (Fase 6, item 12 da análise
//! de 2026-11-12): [`http`] é o cliente compartilhado e [`repo_version`] a
//! versão dos pacotes baixados de repositórios do GitHub.

pub mod http;
pub mod repo_version;
