/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    tick: u32,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            App::default()
        }
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("This is a heading");
            ui.label("This is a label");
            ui.horizontal(|ui| {
                ui.heading("This is a heading inside a horizontal container");
                ui.label("This is a label inside a horizontal container");
                ui.label("This too");
                let button = ui.button("Click me!");
                if button.clicked() {
                    ui.label("Click!");
                }
            });

            ui.label(self.tick.to_string());
            self.tick += 1;

            let (response, painter) =
                ui.allocate_painter(egui::Vec2::new(150.0, 200.0), egui::Sense::empty());
            let rect = response.rect;
            let c = rect.center();
            let r = rect.width() / 2.0 - 10.0;
            painter.circle(
                c,
                r,
                egui::Color32::CYAN,
                egui::Stroke::new(2.0, egui::Color32::MAGENTA),
            );
            ui.label(response.rect.to_string());
            ui.label("Text after painter");
            ctx.request_repaint();
        });
    }
}
