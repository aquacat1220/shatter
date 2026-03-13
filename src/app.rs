use core::f32;

use shatter::*;

// We start to display a warning when we lag more than `ACCEPTABLE_TICK_ERROR` ticks behind.
const ACCEPTABLE_TICK_ERROR: f32 = 5.0;
const SCROLL_DELTA_COEFF: f32 = 0.005;

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
    view_center: math::Vec2,    // The center of view in world coordinates.
    pixels_per_world_unit: f32, // Pixels per world unit. The larger the value is, the tighter the viewbox gets, the *closer* we see.
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let mut world: World = Default::default();
        world
            .add_body(
                math::Vec2::new(0.0, -1.0),
                math::Vec2::new(0.2, 0.0),
                10.0,
                math::Shape::Circle(math::Circle::new(0.5).unwrap()),
            )
            .unwrap();
        world
            .add_body(
                math::Vec2::new(1.0, 0.0),
                math::Vec2::new(0.0, 0.2),
                1.0,
                math::Shape::Circle(math::Circle::new(0.2).unwrap()),
            )
            .unwrap();
        world
            .add_body(
                math::Vec2::new(0.0, 1.0),
                math::Vec2::new(0.2, -0.2),
                5.0,
                math::Shape::Circle(math::Circle::new(0.3).unwrap()),
            )
            .unwrap();
        world
            .add_body(
                math::Vec2::new(5.0, 2.0),
                math::Vec2::new(-0.6, 0.0),
                15.0,
                math::Shape::Circle(math::Circle::new(1.0).unwrap()),
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
            view_center: math::Vec2::ZERO,
            pixels_per_world_unit: 100.0,
        }
    }

    fn screen_delta_to_world(&self, screen_dir: egui::Vec2) -> math::Vec2 {
        let egui::Vec2 { x: dx, y: dy } = screen_dir;
        math::Vec2::new(dx, -dy) * (1.0 / self.pixels_per_world_unit)
    }

    fn world_delta_to_screen(&self, world_dir: math::Vec2) -> egui::Vec2 {
        let math::Vec2 { x: dx, y: dy } = world_dir;
        egui::Vec2::new(dx, -dy) * self.pixels_per_world_unit
    }

    /// Draw the control panel to the current egui context, and return if the user requested to "step".
    fn draw_control_panel(&mut self, ctx: &egui::Context) -> bool {
        let mut step_requested = false;

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
                    // Clear `self.accumulated_world_dt` when we change settings.
                    if response1.changed() || response2.changed() {
                        self.accumulated_world_dt = 0.0;
                    }
                    ui.label(format!("Current Tick: {}", self.tick));
                    let dt = 1.0 / self.ticks_per_second;
                    if self.accumulated_world_dt > ACCEPTABLE_TICK_ERROR * dt {
                        // Small errors can naturally occur, and will usually be fixed on the next frame.
                        // So we only warn when the accumulated error gets larger than 5 ticks.
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
                                step_requested = true;
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
        step_requested
    }

    fn draw_world_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            let rect = response.rect;
            let screen_center = rect.center();

            // Dragging with left click, middle (scroll) click, or touch will pan the world view.
            if response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Middle)
            {
                let screen_drag_delta = response.drag_delta();
                let world_drag_delta = self.screen_delta_to_world(screen_drag_delta);
                self.view_center -= world_drag_delta;
            }

            // Scrolling while hovering will zoom in/out.
            if response.hovered() {
                ui.input(|input| {
                    let zoom = SCROLL_DELTA_COEFF * input.smooth_scroll_delta.y
                        + input
                            .multi_touch()
                            .map_or(1.0, |multi_touch_info| multi_touch_info.zoom_delta);
                    self.pixels_per_world_unit *= zoom;
                });
            }

            let color = ctx.style().visuals.warn_fg_color;

            for body in self.world.bodies() {
                let world_pos = body.position();
                match body.shape() {
                    math::Shape::Circle(circle) => {
                        let world_r = circle.radius;
                        let screen_pos = screen_center
                            + self.world_delta_to_screen(world_pos - self.view_center);
                        let screen_r = self.pixels_per_world_unit * world_r;
                        painter.circle(screen_pos, screen_r, color, egui::Stroke::NONE);
                    }
                }
            }
        });
    }

    fn tick_engine(&mut self) {
        let dt = 1.0 / self.ticks_per_second;
        self.events_last_tick = self.engine.tick(&mut self.world, dt);
        self.tick += 1;
        self.accumulated_world_dt -= dt;
    }
}

impl eframe::App for App {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_theme(egui::Theme::Dark);

        let tick_this_frame = self.draw_control_panel(ctx);
        self.draw_world_view(ctx);

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
                if self.accumulated_world_dt <= 1.0 / self.ticks_per_second {
                    break;
                }
                self.tick_engine();
            }
        } else if tick_this_frame {
            self.tick_engine();
        }
        ctx.request_repaint();
    }
}
