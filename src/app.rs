use core::f32;

use shatter::*;

#[derive(Debug)]
pub struct App {
    tick: u32,
    ticks_per_second: f32,
    speed: f32, // speed multiplier
    paused: bool,
    max_ticks_per_frame: u32,
    accumulated_world_dt: f32,
    world: World,
    engine: Engine,
    events_last_tick: Vec<Event>,
    view_center: (f32, f32), // The center of view in world coordinates.
    view_scale: f32, // Pixels per world unit. The larger the value is, the tighter the viewbox gets, the *closer* we see.
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let mut world: World = Default::default();
        world
            .add_body(
                math::Vec2::new(0.0, 0.0),
                math::Vec2::new(0.1, 0.0),
                1.0,
                math::Shape::Circle(math::Circle::new(0.25).unwrap()),
            )
            .unwrap();
        world
            .add_body(
                math::Vec2::new(1.0, 0.0),
                math::Vec2::new(0.0, 0.1),
                1.0,
                math::Shape::Circle(math::Circle::new(0.15).unwrap()),
            )
            .unwrap();
        world
            .add_body(
                math::Vec2::new(0.0, 1.0),
                math::Vec2::new(0.1, -0.1),
                1.0,
                math::Shape::Circle(math::Circle::new(0.15).unwrap()),
            )
            .unwrap();
        world
            .add_body(
                math::Vec2::new(2.0, 2.0),
                math::Vec2::new(-0.1, 0.0),
                1.0,
                math::Shape::Circle(math::Circle::new(0.35).unwrap()),
            )
            .unwrap();
        App {
            tick: 0,
            ticks_per_second: 100.0,
            speed: 1.0,
            paused: false,
            max_ticks_per_frame: 1,
            accumulated_world_dt: 0.0,
            world,
            engine: Default::default(),
            events_last_tick: vec![],
            view_center: (0.0, 0.0),
            view_scale: 100.0,
        }
    }
}

impl eframe::App for App {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_theme(egui::Theme::Dark);
        let mut step_this_tick = false;

        // Draw the control panel window first.
        let control_panel = egui::Window::new("🗖 Control Panel")
            .hscroll(true)
            .vscroll(true)
            .fixed_size(egui::Vec2::new(300.0, 500.0));
        control_panel.show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                // Statistics section.
                ui.group(|ui| {
                    ui.heading("Statistics");
                    let response1 = ui.add(
                        egui::Slider::new(&mut self.ticks_per_second, 1.0..=10000.0)
                            .text("ticks / second")
                            .logarithmic(true),
                    );
                    let response2 = ui.add(
                        egui::Slider::new(&mut self.speed, 0.01..=100.0)
                            .text("speed multiplier")
                            .logarithmic(true),
                    );
                    if response1.changed() || response2.changed() {
                        self.accumulated_world_dt = 0.0;
                    }
                    ui.label(format!("Current Tick: {}", self.tick));
                    let dt = 1.0 / self.ticks_per_second;
                    if self.accumulated_world_dt > 2.0 * dt {
                        // 1 tick errors can naturally occur, and will usually be fixed on the next frame.
                        ui.label(
                            egui::RichText::new(format!(
                                "Lagging {} ticks",
                                (self.accumulated_world_dt / dt).floor()
                            ))
                            .strong(),
                        );
                        ui.label(egui::RichText::new("Consider lowering the tickrate.").strong());
                    }
                });
                // Playback control section.
                ui.group(|ui| {
                    ui.heading("Playback");
                    ui.with_layout(
                        egui::Layout::top_down_justified(egui::Align::Center),
                        |ui| {
                            if ui
                                .add(
                                    egui::Button::new(if self.paused {
                                        "continue ▶"
                                    } else {
                                        "pause ⏸"
                                    })
                                    .min_size(egui::Vec2::new(0.0, 20.0))
                                    .selected(self.paused),
                                )
                                .clicked()
                            {
                                self.paused = !self.paused;
                            }
                            if ui
                                .add(
                                    egui::Button::new("step ⏭")
                                        .min_size(egui::Vec2::new(0.0, 20.0)),
                                )
                                .clicked()
                            {
                                step_this_tick = true;
                            }
                        },
                    );
                });
                // Event display section.
                ui.group(|ui| {
                    ui.heading(format!("Events ({})", self.events_last_tick.len()));
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                        )
                        .show(ui, |ui| {
                            for event in self.events_last_tick.iter() {
                                ui.group(|ui| {
                                    ui.label(format!("Event details: {event:?}"));
                                });
                            }
                        });
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::empty());
            let rect = response.rect;
            let screen_center = rect.center();

            let color = ctx.style().visuals.warn_fg_color;

            for body in self.world.bodies() {
                let world_pos = body.position();
                match body.shape() {
                    math::Shape::Circle(circle) => {
                        let world_r = circle.radius;
                        let screen_pos = screen_center
                            + self.view_scale
                                * egui::Vec2::new(
                                    world_pos.x - self.view_center.0,
                                    -world_pos.y + self.view_center.1,
                                );
                        let screen_r = self.view_scale * world_r;
                        painter.circle(screen_pos, screen_r, color, egui::Stroke::NONE);
                    }
                }
            }
        });

        let dt = 1.0 / self.ticks_per_second;

        if !self.paused {
            let real_dt = ctx.input(|i| i.unstable_dt);
            let world_dt = real_dt * self.speed;
            self.accumulated_world_dt += world_dt;
            // Under normal conditions, `world_dt` should be smaller than `dt`, and we will need no more than a single engine-tick per frame.
            // For higher tickrates (= higher `self.speed * self.ticks_per_second`), `world_dt` might be larger than `dt`, and we might need more than one engine-tick per frame.
            // If it gets too high, we might need to simulate more ticks than the engine can possibly serve.
            // That's when we need to adjust the tickrate.

            // Here, we allow at max `self.max_ticks_per_frame` simulations each frame. Defaults to 1.
            // If `self.accumulated_world_dt` is larger than `dt` (and grows even more), that might mean the engine isn't fast enough.

            for _ in 0..=self.max_ticks_per_frame {
                if self.accumulated_world_dt <= dt {
                    break;
                }
                self.events_last_tick = self.engine.tick(&mut self.world, dt);
                self.tick += 1;
                self.accumulated_world_dt -= dt;
            }
        } else if step_this_tick {
            self.events_last_tick = self.engine.tick(&mut self.world, dt);
            self.tick += 1;
        }
        ctx.request_repaint();
    }
}
