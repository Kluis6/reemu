-- Versão do core (`retro_system_info.library_version`) que gravou o save
-- state. O formato do state é do core: depois de uma atualização ele pode
-- não carregar, e a interface avisa antes. NULL = gravado antes deste campo.
ALTER TABLE save_states ADD COLUMN core_version TEXT;
