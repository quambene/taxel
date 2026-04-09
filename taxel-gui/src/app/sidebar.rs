use eframe::{
    self,
    egui::{Color32, Frame, Label, Margin, Panel, RichText, ScrollArea, Sense, Ui},
};
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
            ScrollArea::vertical().show(ui, |ui| {
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

                    draw_row(ui, title, i, selected);
                }
            });
        });
}

/// Draw a single row in the sidebar for a report section. Highlights the row if
/// it is selected, and handles click interactions to select the section.
fn draw_row(ui: &mut Ui, title: &str, i: usize, selected: &mut usize) {
    let is_selected = *selected == i;

    let row = Frame::new()
        .fill(if is_selected {
            ui.visuals().selection.bg_fill
        } else {
            Color32::TRANSPARENT
        })
        .corner_radius(2.0)
        .inner_margin(Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let text = if is_selected {
                RichText::new(title).color(ui.visuals().selection.stroke.color)
            } else {
                RichText::new(title)
            };
            ui.add(Label::new(text).wrap());
        });

    let response = ui.interact(
        row.response.rect,
        ui.id().with(("sidebar_section", i)),
        Sense::click(),
    );

    if response.is_pointer_button_down_on() && !is_selected {
        ui.painter().rect_filled(
            row.response.rect,
            2.0,
            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
        );
    } else if response.hovered() && !is_selected {
        ui.painter().rect_filled(
            row.response.rect,
            2.0,
            ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.25),
        );
    }

    if response.clicked() {
        *selected = i;
    }
}
