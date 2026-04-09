use eframe::egui::{self, Panel, Ui};
use taxel::{GCD_LABEL, GCD_ROLE_URI};
use taxel_gui::FactSection;

/// Draw the sidebar panel containing the list of sections. Allows the user to
/// select a section to view its facts in the main table.
pub(super) fn draw_sidebar(
    ctx: &mut Ui,
    sections: &[FactSection],
    selected: &mut usize,
    lang: &str,
) {
    Panel::left("sections_panel")
        .resizable(true)
        .default_size(200.0)
        .show_inside(ctx, |ui| {
            // Match the spacing above the first section in the main table for
            // visual alignment.
            ui.add_space(7.0);
            ui.label("Report sections");
            ui.add_space(2.0);

            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, section) in sections.iter().enumerate() {
                    let title = if section.role == GCD_ROLE_URI {
                        GCD_LABEL
                    } else {
                        section
                            .labels
                            .get(lang)
                            .map(|label| label.as_str())
                            .unwrap_or_else(|| {
                                section.role.rsplit('/').next().unwrap_or(&section.role)
                            })
                    };

                    ui.selectable_value(selected, i, title);
                }
            });
        });
}
