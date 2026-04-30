use eframe::egui;

pub(crate) fn toggle(on: &mut bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| toggle_ui(ui, on)
}

pub(crate) fn toggle_ui(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(1.85, 0.92);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
    });

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let visuals = ui.visuals();
        let stroke = if response.hovered() {
            visuals.widgets.hovered.bg_stroke
        } else {
            visuals.widgets.inactive.bg_stroke
        };
        let bg_fill = if *on {
            visuals.selection.bg_fill
        } else {
            visuals.widgets.inactive.bg_fill
        };
        let rect = rect.expand(1.0);
        let radius = 0.5 * rect.height();

        ui.painter()
            .rect(rect, radius, bg_fill, stroke, egui::StrokeKind::Inside);

        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        let knob_fill = if *on {
            egui::Color32::WHITE
        } else {
            visuals.widgets.inactive.fg_stroke.color
        };
        ui.painter().circle(
            center,
            0.66 * radius,
            knob_fill,
            egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_fill),
        );
    }

    response
}
