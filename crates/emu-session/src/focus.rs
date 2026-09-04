use crate::session::EmuSession;
use domain::focus::{FocusManager, InputFocus};
use std::sync::Arc;

/// `FocusManager` do desktop: alterna `GameFocused <-> MenuFocused` e, na
/// transição, pausa/resume a `EmuSession` (emulação + produção de áudio).
///
/// Decisão do design: entrar em `MenuFocused` **pausa** o core; o menu
/// Fluent fica sempre sobreposto (a WebView continua viva, só sem captar
/// input do jogo). O estado real vive aqui, no lado Rust — o React só
/// espelha via evento.
pub struct FocusController {
    focus: InputFocus,
    session: Arc<EmuSession>,
}

impl FocusController {
    pub fn new(session: Arc<EmuSession>) -> Self {
        Self {
            focus: InputFocus::GameFocused,
            session,
        }
    }

    /// Força um estado específico (ex: erro bloqueante exige `MenuFocused`).
    pub fn set(&mut self, focus: InputFocus) {
        if focus != self.focus {
            self.focus = focus;
            self.apply();
        }
    }

    fn apply(&self) {
        let menu = self.focus == InputFocus::MenuFocused;
        self.session.set_paused(menu);
        self.session.set_game_focused(!menu);
        // Solta tudo na transição — senão o jogo volta com teclas "grudadas" e
        // o resolvedor de hotkey continua vendo a combinação que abriu o menu.
        crate::retropad().clear();
        input_desktop::held::clear();
    }
}

impl FocusManager for FocusController {
    fn current(&self) -> InputFocus {
        self.focus
    }

    fn toggle(&mut self) {
        self.focus = match self.focus {
            InputFocus::GameFocused => InputFocus::MenuFocused,
            InputFocus::MenuFocused => InputFocus::GameFocused,
        };
        self.apply();
    }
}
