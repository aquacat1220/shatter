use shatter::*;

#[derive(Debug, Default)]
pub struct App {
    tick: u32,
    world: World,
    engine: Engine,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        Default::default()
    }
}

impl eframe::App for App {
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

            for handle in self.world.body_handles() {
                let body = self.world.body(handle).unwrap();
                let pos = body.position();
                match body.shape() {
                    math::Shape::Circle(circle) => {
                        let r = circle.radius;
                        painter.circle(
                            egui::Pos2::new(c.x + pos.x, c.y + pos.y),
                            r,
                            egui::Color32::CYAN,
                            egui::Stroke::new(2.0, egui::Color32::MAGENTA),
                        );
                    }
                }
            }

            ui.label(response.rect.to_string());
            ui.label("Text after painter");

            self.engine.tick(&mut self.world, 0.1);
            ctx.request_repaint();
        });
    }
}
