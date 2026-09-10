-- Edição manual dos dados de uma ROM (nome). A plataforma é editada
-- atualizando `roms.system_id` direto — o scan dedup por `file_path` e nunca
-- reescreve linha existente, então não clobbra.
ALTER TABLE roms ADD COLUMN user_title TEXT;
