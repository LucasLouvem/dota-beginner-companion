//! Estilo próprio da aba Herói. Só cores nomeadas do libadwaita, para seguir
//! a aparência clara ou escura do sistema, e classes com o prefixo `dota-`, para
//! não afetar os outros widgets do aplicativo.

/// Folha de estilo do aplicativo.
pub const CSS: &str = "
.dota-card {
    background-color: @card_bg_color;
    color: @card_fg_color;
    border-radius: 8px;
    border: 1px solid alpha(currentColor, 0.12);
}
.dota-card-title {
    font-weight: bold;
}
.dota-build-row:selected {
    box-shadow: inset 4px 0 0 0 @accent_bg_color;
}
.dota-chip-done {
    border-bottom: 3px solid @success_color;
    border-radius: 6px;
}
.dota-chip-next {
    border: 2px solid @accent_color;
    border-radius: 8px;
    background-color: alpha(@accent_bg_color, 0.15);
}
.dota-chip-pending {
    opacity: 0.55;
}
.dota-stat-value {
    font-size: 115%;
    font-weight: 800;
}
button.dota-match-active {
    background-image: none;
    background-color: @success_bg_color;
    color: @success_fg_color;
}
.dota-kda {
    font-weight: bold;
}
.dota-chip-label {
    padding: 3px 10px;
    border-radius: 999px;
    background-color: alpha(currentColor, 0.1);
}
";

/// Registra a folha de estilo. Só pode ser chamada com a tela já aberta, ou
/// seja, depois de o GTK iniciar. A prioridade é a do usuário (a mesma do
/// `~/.config/gtk-4.0/gtk.css`), porque um estilo pessoal costuma estilizar os
/// botões com prioridade mais alta que a do aplicativo e esconderia as cores
/// de estado; as regras só usam classes `dota-`, então não mudam o resto.
pub fn install() {
    relm4::set_global_css_with_priority(CSS, relm4::gtk::STYLE_PROVIDER_PRIORITY_USER);
}
