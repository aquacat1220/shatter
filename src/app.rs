use core::f32;

use shatter::*;

// We start to display a warning when we lag more than `ACCEPTABLE_TICK_ERROR` ticks behind.
const ACCEPTABLE_TICK_ERROR: f32 = 5.0;
const SCROLL_DELTA_COEFF: f32 = 0.005;
const CONTROL_PANEL_SIZE: egui::Vec2 = egui::Vec2::new(300.0, 500.0);
const BODY_EDITOR_GIZMO_SIZE: f32 = 200.0;

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
    body_to_edit: Option<BodyHandle>,
    body_edit_mode: EditMode,
}

#[derive(Debug, Clone, Copy)]
enum EditMode {
    None,
    Pos,
    X,
    Y,
    Vel,
}

impl App {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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
            body_to_edit: None,
            body_edit_mode: EditMode::None,
        }
    }

    fn screen_delta_to_world(screen_delta: egui::Vec2, pixels_per_world_unit: f32) -> math::Vec2 {
        let egui::Vec2 { x: dx, y: dy } = screen_delta;
        math::Vec2::new(dx, -dy) * (1.0 / pixels_per_world_unit)
    }

    fn world_delta_to_screen(world_delta: math::Vec2, pixels_per_world_unit: f32) -> egui::Vec2 {
        let math::Vec2 { x: dx, y: dy } = world_delta;
        egui::Vec2::new(dx, -dy) * pixels_per_world_unit
    }

    /// Draw the control panel to the current egui context, and return if the user requested to "step".
    fn draw_control_panel(&mut self, ctx: &egui::Context) -> bool {
        let mut step_requested = false;

        let control_panel = egui::Window::new("🗖 Control Panel")
            .hscroll(true)
            .vscroll(true)
            .fixed_size(CONTROL_PANEL_SIZE);
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
            let response = ui.interact(
                ui.clip_rect(),
                ui.next_auto_id(),
                egui::Sense::click_and_drag(),
            );
            ui.skip_ahead_auto_ids(1);
            let world_view_rect = response.rect;
            let screen_center = world_view_rect.center();

            // Left clicking a body will select it for editing.
            if response.clicked_by(egui::PointerButton::Primary) {
                ui.input(|input| {
                    let click_pos = input.pointer.interact_pos().unwrap();
                    let world_click_pos = self.view_center
                        + Self::screen_delta_to_world(
                            click_pos - screen_center,
                            self.pixels_per_world_unit,
                        );
                    let query = self.world.query_point(world_click_pos);
                    self.body_to_edit = query;
                });
            }

            // Dragging with left click, middle (scroll) click, or touch will pan the world view.
            if response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Middle)
            {
                let screen_drag_delta = response.drag_delta();
                let world_drag_delta =
                    Self::screen_delta_to_world(screen_drag_delta, self.pixels_per_world_unit);
                self.view_center -= world_drag_delta;
            }

            // Scrolling while hovering or multi-touch-pinching will zoom in/out.
            // Pinching takes priority.
            if response.hovered() {
                ui.input(|input| {
                    let mut zoom = 2.0_f32.powf(SCROLL_DELTA_COEFF * input.smooth_scroll_delta.y);
                    let mut zoom_center = input.pointer.interact_pos();
                    if let Some(multi_touch_info) = input.multi_touch() {
                        zoom = multi_touch_info.zoom_delta;
                        zoom_center = Some(multi_touch_info.center_pos);
                    }
                    // let clamped_zoom = f32::clamp(zoom, 0.5, 1.5);
                    // To ensure we zoom "towards the cursor", move the view center too.
                    let world_zoom_delta = Self::screen_delta_to_world(
                        zoom_center.unwrap_or(screen_center) - screen_center,
                        self.pixels_per_world_unit,
                    );
                    self.view_center += world_zoom_delta * (1.0 - (1.0 / zoom));
                    self.pixels_per_world_unit *= zoom;
                });
            }

            // Fetch colors for the bodies / gizmos.
            let body_color = ctx.style().visuals.weak_text_color();
            let selected_body_color = ctx.style().visuals.text_color();

            let painter = ui.painter();
            // Render bodies in the world view.
            for body_handle in self.world.body_handles() {
                let body = self.world.body(body_handle).unwrap();
                let world_pos = body.position();
                let body_color = if Some(body.handle()) == self.body_to_edit {
                    selected_body_color
                } else {
                    body_color
                };
                match body.shape() {
                    math::Shape::Circle(circle) => {
                        let world_r = circle.radius;
                        let screen_pos = screen_center
                            + Self::world_delta_to_screen(
                                world_pos - self.view_center,
                                self.pixels_per_world_unit,
                            );
                        let screen_r = self.pixels_per_world_unit * world_r;
                        painter.circle(screen_pos, screen_r, body_color, egui::Stroke::NONE);
                    }
                }
            }

            self.draw_body_gizmo(ctx, ui);
        });
    }

    fn draw_body_gizmo(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.body_to_edit.is_none() {
            return;
        }
        let body_to_edit = self.body_to_edit.unwrap();
        // If we have a body selected for edit, draw a gizmo on top of it.
        if let Ok(mut body_to_edit) = self.world.body_mut(body_to_edit) {
            let body_screen_pos = ui.clip_rect().center()
                + Self::world_delta_to_screen(
                    body_to_edit.position() - self.view_center,
                    self.pixels_per_world_unit,
                );

            let pos_controller_width = BODY_EDITOR_GIZMO_SIZE / 16.0;
            let arrow_width = BODY_EDITOR_GIZMO_SIZE / 48.0;
            let arrow_head_size = BODY_EDITOR_GIZMO_SIZE / 24.0;
            let vel_controller_width = pos_controller_width;
            let vel_arrow_width = arrow_width;

            // Compute bounding boxes for gizmo components.
            let gizmo_rect = egui::Rect::from_center_size(
                body_screen_pos,
                egui::Vec2::splat(BODY_EDITOR_GIZMO_SIZE),
            );
            let pos_controller_rect = egui::Rect::from_center_size(
                body_screen_pos,
                egui::Vec2::splat(pos_controller_width),
            );
            let mut y_arrow_rect = egui::Rect::from_center_size(
                body_screen_pos,
                egui::Vec2::splat(arrow_head_size * 2.0),
            );
            y_arrow_rect.extend_with_y(gizmo_rect.top());
            let mut x_arrow_rect = egui::Rect::from_center_size(
                body_screen_pos,
                egui::Vec2::splat(arrow_head_size * 2.0),
            );
            x_arrow_rect.extend_with_x(gizmo_rect.right());
            let body_screen_pos_after_1s = ui.clip_rect().center()
                + Self::world_delta_to_screen(
                    body_to_edit.position() + body_to_edit.velocity() - self.view_center,
                    self.pixels_per_world_unit,
                );
            let vel_controller_rect = egui::Rect::from_center_size(
                body_screen_pos_after_1s,
                egui::Vec2::splat(vel_controller_width),
            );

            // Enable and disable sense based on latest pointer position.
            let mut sense = egui::Sense::drag();
            ui.input(|input| {
                if let Some(last_pointer_pos) = input.pointer.latest_pos()
                    && !input.any_touches()
                {
                    // If we are not on a touch device and have a valid last pointer pos, use it to disable senses when unnecessary.
                    // This is to allow inputs to pass through transparent regions of the gizmo and affect the world view underneath.
                    if !pos_controller_rect.contains(last_pointer_pos)
                        && !y_arrow_rect.contains(last_pointer_pos)
                        && !x_arrow_rect.contains(last_pointer_pos)
                    {
                        sense = egui::Sense::empty();
                    }
                }
            });

            // Fetch responses and set body edit mode.
            let response_gizmo = ui.interact(gizmo_rect, ui.next_auto_id(), sense);
            ui.skip_ahead_auto_ids(1);
            if response_gizmo.drag_started_by(egui::PointerButton::Primary) {
                // We have to determine where on the gizmo the drag started.
                let mut interact_pos: Option<egui::Pos2> = None;
                ui.input(|input| {
                    interact_pos = input.pointer.interact_pos();
                });
                let interact_pos = interact_pos.unwrap_or(body_screen_pos);
                let delta_pos = interact_pos - body_screen_pos;
                if delta_pos.length() <= (pos_controller_width / 2.0) {
                    // If the drag start pos was close to the position controller, it takes priority.
                    self.body_edit_mode = EditMode::Pos;
                } else if delta_pos.y.abs() >= delta_pos.x.abs() {
                    // Else if, the drag start pos was closer to the y arrow than the x arrow, y arrow takes priority.
                    self.body_edit_mode = EditMode::Y;
                } else {
                    // x arrow takes remaining drag starts.
                    self.body_edit_mode = EditMode::X;
                }
            }
            if response_gizmo.drag_stopped_by(egui::PointerButton::Primary) {
                self.body_edit_mode = EditMode::None;
            }

            let response_vel_controller =
                ui.interact(vel_controller_rect, ui.next_auto_id(), egui::Sense::drag());
            ui.skip_ahead_auto_ids(1);
            if response_vel_controller.drag_started_by(egui::PointerButton::Primary) {
                self.body_edit_mode = EditMode::Vel;
            }
            if response_vel_controller.drag_stopped_by(egui::PointerButton::Primary) {
                self.body_edit_mode = EditMode::None;
            }

            // Set gizmo color based on edit mode and hover state.
            let mut pos_controller_color = ctx.style().visuals.gray_out(egui::Color32::YELLOW);
            let mut y_arrow_color = ctx.style().visuals.gray_out(egui::Color32::GREEN);
            let mut x_arrow_color = ctx.style().visuals.gray_out(egui::Color32::RED);
            let mut vel_controller_color = ctx.style().visuals.gray_out(egui::Color32::YELLOW);
            match self.body_edit_mode {
                EditMode::Vel => {
                    vel_controller_color = egui::Color32::YELLOW;
                }
                EditMode::Pos => {
                    pos_controller_color = egui::Color32::YELLOW;
                }
                EditMode::Y => {
                    y_arrow_color = egui::Color32::GREEN;
                }
                EditMode::X => {
                    x_arrow_color = egui::Color32::RED;
                }
                EditMode::None => {
                    let mut latest_pos: Option<egui::Pos2> = None;
                    ui.input(|input| {
                        latest_pos = input.pointer.latest_pos();
                    });
                    if let Some(latest_pos) = latest_pos {
                        if pos_controller_rect.contains(latest_pos) {
                            pos_controller_color = egui::Color32::YELLOW;
                        } else if y_arrow_rect.contains(latest_pos) {
                            y_arrow_color = egui::Color32::GREEN;
                        } else if x_arrow_rect.contains(latest_pos) {
                            x_arrow_color = egui::Color32::RED;
                        } else if vel_controller_rect.contains(latest_pos) {
                            vel_controller_color = egui::Color32::YELLOW;
                        }
                    }
                }
            }

            let gizmo_drag_delta = response_gizmo.drag_delta();
            let gizmo_world_drag_delta =
                Self::screen_delta_to_world(gizmo_drag_delta, self.pixels_per_world_unit);
            let vel_controller_drag_delta = response_vel_controller.drag_delta();
            let vel_controller_world_drag_delta =
                Self::screen_delta_to_world(vel_controller_drag_delta, self.pixels_per_world_unit);
            match self.body_edit_mode {
                EditMode::Vel => {
                    println!("{:?}", body_to_edit.velocity());
                    *body_to_edit.velocity_mut() =
                        body_to_edit.velocity() + vel_controller_world_drag_delta;
                    println!("{:?}", body_to_edit.velocity());
                }
                EditMode::Pos => {
                    *body_to_edit.position_mut() += gizmo_world_drag_delta;
                }
                EditMode::Y => {
                    body_to_edit.position_mut().y += gizmo_world_drag_delta.y;
                }
                EditMode::X => {
                    body_to_edit.position_mut().x += gizmo_world_drag_delta.x;
                }
                EditMode::None => {}
            }

            let painter = ui.painter();
            // Paint the +y arrow.
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos_controller_rect.center(),
                    gizmo_rect.center_top(),
                    gizmo_rect.center_top() + egui::Vec2::new(arrow_head_size, arrow_head_size),
                ],
                egui::epaint::PathStroke::new(arrow_width, y_arrow_color),
            )));
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    gizmo_rect.center_top(),
                    gizmo_rect.center_top() + egui::Vec2::new(-arrow_head_size, arrow_head_size),
                ],
                egui::epaint::PathStroke::new(arrow_width, y_arrow_color),
            )));
            // Paint the +x arrow.
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos_controller_rect.center(),
                    gizmo_rect.right_center(),
                    gizmo_rect.right_center() + egui::Vec2::new(-arrow_head_size, arrow_head_size),
                ],
                egui::epaint::PathStroke::new(arrow_width, x_arrow_color),
            )));
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    gizmo_rect.right_center(),
                    gizmo_rect.right_center() + egui::Vec2::new(-arrow_head_size, -arrow_head_size),
                ],
                egui::epaint::PathStroke::new(arrow_width, x_arrow_color),
            )));
            // Paint the velocity controller.
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![pos_controller_rect.center(), vel_controller_rect.center()],
                egui::epaint::PathStroke::new(vel_arrow_width, vel_controller_color),
            )));
            painter.rect_filled(vel_controller_rect, 0.0, vel_controller_color);

            // Paint the 2d position controller. This should come last to show up on the top.
            painter.rect_filled(pos_controller_rect, 0.0, pos_controller_color);
        } else {
            self.body_to_edit = None; // Selected body handle went stale, possibly due to the selected body being removed.
        }
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
