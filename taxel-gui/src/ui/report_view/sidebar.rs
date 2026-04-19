use crate::domain::ReportSection;
use eframe::{
    self,
    egui::{Color32, CursorIcon, Frame, Label, Margin, Panel, RichText, ScrollArea, Sense, Ui},
};

/// Draw the sidebar panel containing the list of sections. Allows the user to
/// select a section to view its facts in the main table.
pub fn draw_sidebar(ctx: &mut Ui, sections: &[ReportSection], selected: &mut usize, lang: &str) {
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
                if sections.is_empty() {
                    ui.add_space(6.0);
                    ui.weak("Import a report to view sections.");
                    return;
                }

                for (i, section) in sections.iter().enumerate() {
                    let title = section
                        .labels
                        .get(lang)
                        .map(|label| label.as_str())
                        .unwrap_or_else(|| {
                            section.role.rsplit('/').next().unwrap_or(&section.role)
                        });

                    draw_row(ui, title, i, selected, section.disabled);
                }
            });
        });
}

/// Draw a single row in the sidebar for a report section. Highlights the row if
/// it is selected, and handles click interactions to select the section.
fn draw_row(ui: &mut Ui, title: &str, i: usize, selected: &mut usize, disabled: bool) {
    let is_selected = *selected == i;

    let row = Frame::new()
        .fill(if is_selected && !disabled {
            ui.visuals().selection.bg_fill
        } else {
            Color32::TRANSPARENT
        })
        .corner_radius(2.0)
        .inner_margin(Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let text = if disabled {
                RichText::new(title).color(ui.visuals().weak_text_color())
            } else if is_selected {
                RichText::new(title).color(ui.visuals().selection.stroke.color)
            } else {
                RichText::new(title)
            };
            ui.add(Label::new(text).wrap());
        });

    if disabled {
        return;
    }

    let response = ui
        .interact(
            row.response.rect,
            ui.id().with(("sidebar_section", i)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);

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
