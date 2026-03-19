use core::f32;

use egui_taffy::{TuiBuilderLogic, taffy, tui};
use shatter::*;

// We start to display a warning when we lag more than `ACCEPTABLE_TICK_ERROR` ticks behind.
const ACCEPTABLE_TICK_ERROR: f32 = 5.0;
const SCROLL_DELTA_COEFF: f32 = 0.005;

const ENGINE_CONTROLLER_SIZE: egui::Vec2 = egui::Vec2::new(300.0, 500.0);

const BODY_EDITOR_GIZMO_SIZE: f32 = 200.0;
const BODY_EDITOR_PANEL_WIDTH: f32 = 300.0;
const BODY_EDITOR_PANEL_OFFSET: egui::Vec2 = egui::Vec2::splat(-50.0);

const BODY_CREATOR_SIZE: egui::Vec2 = egui::Vec2::new(500.0, 500.0);
const BODY_CREATOR_PANEL_WIDTH: f32 = 250.0;
const BODY_CREATOR_PANEL_OFFSET: egui::Vec2 = egui::Vec2::splat(-50.0);

const AXES_WIDTH: f32 = BODY_EDITOR_GIZMO_SIZE / 48.0;
const AXES_HEAD_WIDTH: f32 = BODY_EDITOR_GIZMO_SIZE / 24.0;
const HANDLE_WIDTH: f32 = AXES_WIDTH;
const HANDLE_HEAD_WIDTH: f32 = BODY_EDITOR_GIZMO_SIZE / 16.0;

#[derive(Debug)]
pub struct App {
    // Begin engine configuration.
    engine: Engine,
    tick: u32,
    ticks_per_second: f32,
    speed: f32, // speed multiplier
    paused: bool,
    max_ticks_per_frame: u32,
    accumulated_world_dt: f32,
    // End engine configuration.
    // Begin world configuration.
    world: World,
    events_last_tick: Vec<Event>,
    view_center: math::Vec2,    // The center of view in world coordinates.
    pixels_per_world_unit: f32, // Pixels per world unit. The larger the value is, the tighter the viewbox gets, the *closer* we see.
    body_to_edit: Option<BodyHandle>,
    // End world configuration.
    // Begin body creator world configuration.
    body_creator_world: World,
    pixels_per_body_creator_world_unit: f32,
    // TODO: After adding multi-collider support, we will need an `Option<ColliderHandle>` to keep track of the target collider.
    // End body creator world configuration.
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

        let mut body_creator_world: World = Default::default();
        body_creator_world
            .add_body(
                math::Vec2::ZERO,
                math::Vec2::ZERO,
                1.0,
                math::Shape::Circle(math::Circle::new(1.0).unwrap()),
            )
            .unwrap();
        App {
            engine: Default::default(),
            tick: 0,
            ticks_per_second: 100.0,
            speed: 1.0,
            paused: false,
            max_ticks_per_frame: 1,
            accumulated_world_dt: 0.0,
            world,
            events_last_tick: vec![],
            view_center: math::Vec2::ZERO,
            pixels_per_world_unit: 100.0,
            body_to_edit: None,
            body_creator_world,
            pixels_per_body_creator_world_unit: 100.0,
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

    /// Draw the engine controller to the current egui context, and return if the user requested to "step".
    fn draw_engine_controller(&mut self, ctx: &egui::Context) -> bool {
        let mut step_requested = false;

        let engine_controller = egui::Window::new("⚙ Engine Controller")
            .hscroll(true)
            .vscroll(true)
            .fixed_size(ENGINE_CONTROLLER_SIZE);
        engine_controller.show(ctx, |ui| {
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

    fn draw_world_viewer(&mut self, ctx: &egui::Context) {
        let _response = egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.interact(
                ui.clip_rect(),
                ui.next_auto_id(),
                egui::Sense::click_and_drag(),
            );
            ui.skip_ahead_auto_ids(1);
            let screen_center = ui.clip_rect().center();

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
            let body_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 255);
            let selected_body_color = egui::Color32::from_rgba_unmultiplied(168, 168, 168, 255);
            let grid_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 255);

            Self::draw_world(
                ui,
                &self.world,
                self.view_center,
                self.pixels_per_world_unit,
                self.body_to_edit,
                body_color,
                selected_body_color,
                grid_color,
            );

            self.draw_body_editor_gizmo(ui);

            response
        });

        // if response.inner.hovered() {
        self.draw_body_editor_panel(ctx);
        // }
    }

    fn draw_body_editor_gizmo(&mut self, ui: &mut egui::Ui) {
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
            let body_screen_vel =
                Self::world_delta_to_screen(body_to_edit.velocity(), self.pixels_per_world_unit);

            let position_screen_delta = Self::draw_axes(
                ui,
                body_screen_pos,
                egui::Vec2::splat(BODY_EDITOR_GIZMO_SIZE),
                AXES_WIDTH,
                AXES_HEAD_WIDTH,
                egui::Color32::GREEN * egui::Color32::GRAY,
                egui::Color32::GREEN,
                egui::Color32::RED * egui::Color32::GRAY,
                egui::Color32::RED,
            );
            let velocity_screen_delta = Self::draw_handle(
                ui,
                body_screen_pos,
                body_screen_vel,
                HANDLE_WIDTH,
                HANDLE_HEAD_WIDTH,
                egui::Color32::YELLOW * egui::Color32::GRAY,
                egui::Color32::YELLOW,
            );
            let position_screen_delta = position_screen_delta
                + Self::draw_handle(
                    ui,
                    body_screen_pos,
                    egui::Vec2::ZERO,
                    HANDLE_WIDTH,
                    HANDLE_HEAD_WIDTH,
                    egui::Color32::YELLOW * egui::Color32::GRAY,
                    egui::Color32::YELLOW,
                );

            *body_to_edit.position_mut() +=
                Self::screen_delta_to_world(position_screen_delta, self.pixels_per_world_unit);
            *body_to_edit.velocity_mut() +=
                Self::screen_delta_to_world(velocity_screen_delta, self.pixels_per_world_unit);
        } else {
            self.body_to_edit = None; // Selected body handle went stale, possibly due to the selected body being removed.
        }
    }

    fn draw_body_editor_panel(&mut self, ctx: &egui::Context) {
        if self.body_to_edit.is_none() {
            return;
        }
        let body_to_edit = self.body_to_edit.unwrap();
        let body_to_edit = self.world.body_mut(body_to_edit);
        if body_to_edit.is_err() {
            self.body_to_edit = None;
            return;
        }
        let mut body_to_edit = body_to_edit.unwrap();
        let body_edit_panel = egui::Window::new("⌖ Body Editor Panel")
            // .fixed_size(BODY_EDITOR_PANEL_SIZE)
            .max_width(BODY_EDITOR_PANEL_WIDTH)
            .title_bar(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, BODY_EDITOR_PANEL_OFFSET)
            .order(egui::Order::Foreground);
        body_edit_panel.show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let id = ui.id().with("body_edit_panel");
            let available_width = ui.available_width();
            tui(ui, id)
                // .reserve_available_space()
                .with_available_space(taffy::Size {
                    width: taffy::AvailableSpace::Definite(available_width),
                    height: taffy::AvailableSpace::MaxContent,
                })
                .style(taffy::Style {
                    flex_direction: taffy::FlexDirection::Column,
                    align_items: Some(taffy::AlignItems::Stretch),
                    justify_content: Some(taffy::AlignContent::Center),
                    size: taffy::Size {
                        width: taffy::style_helpers::percent(1.),
                        height: taffy::style_helpers::auto(),
                    },
                    padding: taffy::style_helpers::length(4.0),
                    gap: taffy::style_helpers::length(4.0),
                    ..Default::default()
                })
                .show(|tui| {
                    // Position row.
                    tui.style(taffy::Style {
                        flex_direction: taffy::FlexDirection::Row,
                        align_items: Some(taffy::AlignItems::Stretch),
                        justify_content: Some(taffy::AlignContent::SpaceBetween),
                        padding: taffy::style_helpers::length(4.0),
                        gap: taffy::style_helpers::length(8.0),
                        ..Default::default()
                    })
                    .add_with_border(|tui| {
                        tui.style(taffy::Style {
                            size: taffy::Size {
                                width: taffy::style_helpers::length(30.0),
                                height: taffy::style_helpers::auto(),
                            },
                            ..Default::default()
                        })
                        .label("x");
                        tui.style(taffy::Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        })
                        .ui_add_manual(
                            |ui| {
                                let drag_value =
                                    egui::DragValue::new(&mut body_to_edit.position_mut().x)
                                        .speed(0.01)
                                        .min_decimals(2);
                                ui.add_sized(ui.available_size(), drag_value)
                            },
                            |mut response, _ui| {
                                let rect = response.min_size;
                                response.min_size = egui::Vec2::new(10.0, rect.y);
                                response.max_size = egui::Vec2::new(10.0, rect.y);
                                response.infinite = egui::Vec2b::new(true, false);
                                response
                            },
                        );
                        tui.separator();
                        tui.style(taffy::Style {
                            size: taffy::Size {
                                width: taffy::style_helpers::length(30.0),
                                height: taffy::style_helpers::auto(),
                            },
                            ..Default::default()
                        })
                        .label("y");
                        tui.style(taffy::Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        })
                        .ui_add_manual(
                            |ui| {
                                let drag_value =
                                    egui::DragValue::new(&mut body_to_edit.position_mut().y)
                                        .speed(0.01)
                                        .min_decimals(2);
                                ui.add_sized(ui.available_size(), drag_value)
                            },
                            |mut response, _ui| {
                                let rect = response.min_size;
                                response.min_size = egui::Vec2::new(10.0, rect.y);
                                response.max_size = egui::Vec2::new(10.0, rect.y);
                                response.infinite = egui::Vec2b::new(true, false);
                                response
                            },
                        );
                    });
                    // Velocity row.
                    tui.style(taffy::Style {
                        flex_direction: taffy::FlexDirection::Row,
                        align_items: Some(taffy::AlignItems::Stretch),
                        justify_content: Some(taffy::AlignContent::SpaceBetween),
                        padding: taffy::style_helpers::length(4.0),
                        gap: taffy::style_helpers::length(8.0),
                        ..Default::default()
                    })
                    .add_with_border(|tui| {
                        tui.style(taffy::Style {
                            size: taffy::Size {
                                width: taffy::style_helpers::length(30.0),
                                height: taffy::style_helpers::auto(),
                            },
                            ..Default::default()
                        })
                        .label("vx");
                        tui.style(taffy::Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        })
                        .ui_add_manual(
                            |ui| {
                                let drag_value =
                                    egui::DragValue::new(&mut body_to_edit.velocity_mut().x)
                                        .speed(0.01)
                                        .min_decimals(2);
                                ui.add_sized(ui.available_size(), drag_value)
                            },
                            |mut response, _ui| {
                                let rect = response.min_size;
                                response.min_size = egui::Vec2::new(10.0, rect.y);
                                response.max_size = egui::Vec2::new(10.0, rect.y);
                                response.infinite = egui::Vec2b::new(true, false);
                                response
                            },
                        );
                        tui.separator();
                        tui.style(taffy::Style {
                            size: taffy::Size {
                                width: taffy::style_helpers::length(30.0),
                                height: taffy::style_helpers::auto(),
                            },
                            ..Default::default()
                        })
                        .label("vy");
                        tui.style(taffy::Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        })
                        .ui_add_manual(
                            |ui| {
                                let drag_value =
                                    egui::DragValue::new(&mut body_to_edit.velocity_mut().y)
                                        .speed(0.01)
                                        .min_decimals(2);
                                ui.add_sized(ui.available_size(), drag_value)
                            },
                            |mut response, _ui| {
                                let rect = response.min_size;
                                response.min_size = egui::Vec2::new(10.0, rect.y);
                                response.max_size = egui::Vec2::new(10.0, rect.y);
                                response.infinite = egui::Vec2b::new(true, false);
                                response
                            },
                        );
                    });
                    // Impulse row.
                    tui.style(taffy::Style {
                        flex_direction: taffy::FlexDirection::Row,
                        align_items: Some(taffy::AlignItems::Center),
                        justify_content: Some(taffy::AlignContent::SpaceBetween),
                        padding: taffy::style_helpers::length(4.0),
                        gap: taffy::style_helpers::length(8.0),
                        ..Default::default()
                    })
                    .add_with_border(|tui| {
                        tui.style(taffy::Style {
                            size: taffy::Size {
                                width: taffy::style_helpers::length(30.0),
                                height: taffy::style_helpers::auto(),
                            },
                            ..Default::default()
                        })
                        .label("fx");
                        tui.style(taffy::Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        })
                        .ui_add_manual(
                            |ui| {
                                let drag_value = egui::DragValue::new(
                                    &mut body_to_edit.accumulated_impulse_mut().x,
                                )
                                .speed(0.01)
                                .min_decimals(2);
                                ui.add_sized(ui.available_size(), drag_value)
                            },
                            |mut response, _ui| {
                                let rect = response.min_size;
                                response.min_size = egui::Vec2::new(10.0, rect.y);
                                response.max_size = egui::Vec2::new(10.0, rect.y);
                                response.infinite = egui::Vec2b::new(true, false);
                                response
                            },
                        );
                        tui.separator();
                        tui.style(taffy::Style {
                            size: taffy::Size {
                                width: taffy::style_helpers::length(30.0),
                                height: taffy::style_helpers::auto(),
                            },
                            ..Default::default()
                        })
                        .label("fy");
                        tui.style(taffy::Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        })
                        .ui_add_manual(
                            |ui| {
                                let drag_value = egui::DragValue::new(
                                    &mut body_to_edit.accumulated_impulse_mut().y,
                                )
                                .speed(0.01)
                                .min_decimals(2);
                                ui.add_sized(ui.available_size(), drag_value)
                            },
                            |mut response, _ui| {
                                let rect = response.min_size;
                                response.min_size = egui::Vec2::new(10.0, rect.y);
                                response.max_size = egui::Vec2::new(10.0, rect.y);
                                response.infinite = egui::Vec2b::new(true, false);
                                response
                            },
                        );
                    });
                });
        });
    }

    fn draw_body_creator(&mut self, ctx: &egui::Context) {
        let body_creator = egui::Window::new("⌖ Body Creator")
            .hscroll(true)
            .vscroll(true)
            .fixed_size(BODY_CREATOR_SIZE);
        let response = body_creator
            .show(ctx, |ui| {
                // Fetch input responses to zoom and pan world view.
                let response = ui.interact(
                    ui.clip_rect(),
                    ui.next_auto_id(),
                    egui::Sense::click_and_drag(),
                );
                ui.skip_ahead_auto_ids(1);

                // Scrolling while hovering or multi-touch-pinching will zoom in/out.
                // Pinching takes priority.
                // Note how we don't care about zoom center, cuz the view center is fixed to the origin.
                if response.hovered() {
                    ui.input(|input| {
                        let mut zoom =
                            2.0_f32.powf(SCROLL_DELTA_COEFF * input.smooth_scroll_delta.y);
                        if let Some(multi_touch_info) = input.multi_touch() {
                            zoom = multi_touch_info.zoom_delta;
                        }
                        self.pixels_per_body_creator_world_unit *= zoom;
                    });
                }

                let body_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 255);
                let selected_body_color = egui::Color32::from_rgba_unmultiplied(168, 168, 168, 255);
                let grid_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 255);

                // And use `Self::draw_world()` to draw the world into `ui`.
                Self::draw_world(
                    ui,
                    &self.body_creator_world,
                    math::Vec2::ZERO,
                    self.pixels_per_body_creator_world_unit,
                    None,
                    body_color,
                    selected_body_color,
                    grid_color,
                );

                // And draw gizmos over the world view to change size.
                // Currently we only allow bodies to have a single collider. No more, no less.
                // The user can either select the collider and change its size, or select nothing and edit bounce coefficients and mass.

                let body_handle = self.body_creator_world.body_handles().next().unwrap();
                let mut body_mut = self.body_creator_world.body_mut(body_handle).unwrap();
                let mut size_world = math::Vec2::ONES;
                match body_mut.shape() {
                    math::Shape::Circle(circle) => {
                        size_world *= circle.radius;
                    }
                }
                let size_in_screen = Self::world_delta_to_screen(
                    size_world,
                    self.pixels_per_body_creator_world_unit,
                );
                let size_delta_screen = Self::draw_handle(
                    ui,
                    ui.clip_rect().center(),
                    size_in_screen,
                    HANDLE_WIDTH,
                    HANDLE_HEAD_WIDTH,
                    egui::Color32::YELLOW * egui::Color32::GRAY,
                    egui::Color32::YELLOW,
                );
                let size_delta_world = Self::screen_delta_to_world(
                    size_delta_screen,
                    self.pixels_per_body_creator_world_unit,
                );
                match body_mut.shape_mut() {
                    math::Shape::Circle(circle) => {
                        circle.radius = f32::max(f32::EPSILON, circle.radius + size_delta_world.x);
                    }
                }

                response
            })
            .unwrap(); // Safety: Never `None` because the window cannot be closed.
        let body_creator_window_rect = response.response.rect;

        if let Some(_inner) = response.inner
        // Pattern matching passes only when window wasn't collapsed.
        // && inner.hovered()
        {
            // Draw a numeric panel only when the body creator is not collapsed.
            let body_creator_panel = egui::Window::new("⌖ Body Creator Panel")
                .max_width(BODY_CREATOR_PANEL_WIDTH)
                .title_bar(false)
                .resizable(false)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .fixed_pos(body_creator_window_rect.right_bottom() + BODY_CREATOR_PANEL_OFFSET)
                .order(egui::Order::Foreground);
            body_creator_panel.show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                let id = ui.id().with("body_creator_panel");
                let available_width = ui.available_width();
                let body_handle = self.body_creator_world.body_handles().next().unwrap();
                let mut body_mut = self.body_creator_world.body_mut(body_handle).unwrap();
                tui(ui, id)
                    .with_available_space(taffy::Size {
                        width: taffy::AvailableSpace::Definite(available_width),
                        height: taffy::AvailableSpace::MaxContent,
                    })
                    .style(taffy::Style {
                        flex_direction: taffy::FlexDirection::Column,
                        align_items: Some(taffy::AlignItems::Stretch),
                        justify_content: Some(taffy::AlignContent::Center),
                        size: taffy::Size {
                            width: taffy::style_helpers::percent(1.),
                            height: taffy::style_helpers::auto(),
                        },
                        padding: taffy::style_helpers::length(4.0),
                        gap: taffy::style_helpers::length(4.0),
                        ..Default::default()
                    })
                    .show(|tui| {
                        tui.style(taffy::Style {
                            flex_direction: taffy::FlexDirection::Row,
                            align_items: Some(taffy::AlignItems::Stretch),
                            justify_content: Some(taffy::AlignContent::SpaceBetween),
                            padding: taffy::style_helpers::length(4.0),
                            gap: taffy::style_helpers::length(8.0),
                            ..Default::default()
                        })
                        .add_with_border(|tui| {
                            match body_mut.shape_mut() {
                                math::Shape::Circle(circle) => {
                                    tui.style(taffy::Style {
                                        size: taffy::Size {
                                            width: taffy::style_helpers::length(30.0),
                                            height: taffy::style_helpers::auto(),
                                        },
                                        ..Default::default()
                                    })
                                    .label("r");
                                    tui.style(taffy::Style {
                                        flex_grow: 1.0,
                                        ..Default::default()
                                    })
                                    .ui_add_manual(
                                        |ui| {
                                            let drag_value =
                                                egui::DragValue::new(&mut circle.radius)
                                                    .range(f32::EPSILON..=f32::INFINITY)
                                                    .speed(0.01)
                                                    .min_decimals(2);
                                            ui.add_sized(ui.available_size(), drag_value)
                                        },
                                        |mut response, _ui| {
                                            let rect = response.min_size;
                                            response.min_size = egui::Vec2::new(10.0, rect.y);
                                            response.max_size = egui::Vec2::new(10.0, rect.y);
                                            response.infinite = egui::Vec2b::new(true, false);
                                            response
                                        },
                                    );
                                }
                            }
                        });
                        tui.style(taffy::Style {
                            flex_direction: taffy::FlexDirection::Row,
                            align_items: Some(taffy::AlignItems::Stretch),
                            justify_content: Some(taffy::AlignContent::SpaceBetween),
                            padding: taffy::style_helpers::length(4.0),
                            gap: taffy::style_helpers::length(8.0),
                            ..Default::default()
                        })
                        .add_with_border(|tui| {
                            tui.style(taffy::Style {
                                flex_grow: 1.0,
                                ..Default::default()
                            })
                            .ui_add_manual(
                                |ui| {
                                    let button = egui::Button::new("Add Body To Scene");
                                    let response = ui.add_sized(ui.available_size(), button);
                                    if response.clicked() {
                                        self.world
                                            .add_body(
                                                self.view_center,
                                                math::Vec2::ZERO,
                                                1.0 / body_mut.mass_inv(),
                                                body_mut.shape(),
                                            )
                                            .unwrap();
                                    }
                                    response
                                },
                                |mut response, _ui| {
                                    let rect = response.min_size;
                                    response.min_size = egui::Vec2::new(10.0, rect.y);
                                    response.max_size = egui::Vec2::new(10.0, rect.y);
                                    response.infinite = egui::Vec2b::new(true, false);
                                    response
                                },
                            );
                        });
                    });
            });
        }
    }

    /// Draw the world inside provided ui.
    fn draw_world(
        ui: &mut egui::Ui,
        world: &World,
        view_center: math::Vec2,
        pixels_per_world_unit: f32,
        selected_body: Option<BodyHandle>,
        body_color: egui::Color32,
        selected_body_color: egui::Color32,
        grid_color: egui::Color32,
    ) {
        let rect = ui.clip_rect();
        let painter = ui.painter();
        // Render bodies in the world view.
        for body_handle in world.body_handles() {
            let body = world.body(body_handle).unwrap();
            let world_pos = body.position();
            let body_color = if Some(body.handle()) == selected_body {
                selected_body_color
            } else {
                body_color
            };
            match body.shape() {
                math::Shape::Circle(circle) => {
                    let world_r = circle.radius;
                    let screen_pos = rect.center()
                        + Self::world_delta_to_screen(
                            world_pos - view_center,
                            pixels_per_world_unit,
                        );
                    let screen_r = pixels_per_world_unit * world_r;
                    painter.circle(screen_pos, screen_r, body_color, egui::Stroke::NONE);
                }
            }
        }
        Self::draw_grid(
            ui,
            view_center,
            pixels_per_world_unit,
            250.0,
            egui::Stroke::new(1.0, grid_color),
        );
        Self::draw_grid(
            ui,
            view_center,
            pixels_per_world_unit,
            25.0,
            egui::Stroke::new(0.5, grid_color * egui::Color32::GRAY),
        );
    }

    fn draw_grid(
        ui: &egui::Ui,
        view_center: math::Vec2,
        pixels_per_world_unit: f32,
        target_screen_line_distance: f32,
        stroke: egui::Stroke,
    ) {
        let rect = ui.clip_rect();
        let painter = ui.painter();
        let world_spacing = target_screen_line_distance / pixels_per_world_unit;
        let exponent = world_spacing.log10().ceil();
        let grid_step = 10.0_f32.powf(exponent);

        // 2. Calculate the visible range in world coordinates
        let egui::Vec2 {
            x: width_screen,
            y: height_screen,
        } = rect.size();
        let world_width = width_screen / pixels_per_world_unit;
        let world_height = height_screen / pixels_per_world_unit;

        let world_min = view_center - math::Vec2::new(world_width / 2.0, world_height / 2.0);
        let world_max = view_center + math::Vec2::new(world_width / 2.0, world_height / 2.0);

        // 3. Snap the start points to the grid_step
        let start_x = (world_min.x / grid_step).floor() * grid_step;
        let start_y = (world_min.y / grid_step).floor() * grid_step;

        // Draw Vertical Lines
        let mut x = start_x;
        while x <= world_max.x {
            let dx_screen = (x - view_center.x) * pixels_per_world_unit;
            let screen_x = rect.center().x + dx_screen;
            painter.line_segment(
                [
                    egui::Pos2::new(screen_x, rect.top()),
                    egui::Pos2::new(screen_x, rect.bottom()),
                ],
                stroke,
            );
            x += grid_step;
        }

        // Draw Horizontal Lines
        let mut y = start_y;
        while y <= world_max.y {
            let dy = (y - view_center.y) * pixels_per_world_unit;
            let screen_y = rect.center().y - dy;
            painter.line_segment(
                [
                    egui::Pos2::new(rect.left(), screen_y),
                    egui::Pos2::new(rect.right(), screen_y),
                ],
                stroke,
            );
            y += grid_step;
        }
    }

    fn draw_axes(
        ui: &mut egui::Ui,
        screen_pos: egui::Pos2,
        screen_size: egui::Vec2,
        arrow_width: f32,
        arrow_head_width: f32,
        color_y: egui::Color32,
        color_y_dragged: egui::Color32,
        color_x: egui::Color32,
        color_x_dragged: egui::Color32,
    ) -> egui::Vec2 {
        let mut delta = egui::Vec2::ZERO;
        let mut color_y = color_y;
        let mut color_x = color_x;

        // Compute rects.
        let y_arrow_end = screen_pos + egui::Vec2::UP * screen_size.y.abs();
        let mut y_arrow_rect = egui::Rect::from_center_size(
            y_arrow_end,
            egui::Vec2::splat(f32::max(arrow_width, arrow_head_width)),
        );
        y_arrow_rect.extend_with(screen_pos);
        let x_arrow_end = screen_pos + egui::Vec2::RIGHT * screen_size.x.abs();
        let mut x_arrow_rect = egui::Rect::from_center_size(
            x_arrow_end,
            egui::Vec2::splat(f32::max(arrow_width, arrow_head_width)),
        );
        x_arrow_rect.extend_with(screen_pos);

        // Fetch input responses.
        let id_y = ui.next_auto_id();
        ui.skip_ahead_auto_ids(1);
        let id_x = ui.next_auto_id();
        ui.skip_ahead_auto_ids(1);
        let response_y = ui.interact(y_arrow_rect, id_y, egui::Sense::drag());
        let response_x = ui.interact(x_arrow_rect, id_x, egui::Sense::drag());
        if response_y.dragged_by(egui::PointerButton::Primary) {
            delta.y = response_y.drag_delta().y;
            color_y = color_y_dragged;
        }
        if response_y.hovered() {
            color_y = color_y_dragged;
        }
        if response_x.dragged_by(egui::PointerButton::Primary) {
            delta.x = response_x.drag_delta().x;
            color_x = color_x_dragged;
        }
        if response_x.hovered() {
            color_x = color_x_dragged;
        }

        let painter = ui.painter();
        // Paint the +y arrow to ui.
        painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
            vec![
                screen_pos,
                y_arrow_end,
                y_arrow_end + egui::Vec2::new(arrow_head_width, arrow_head_width),
            ],
            egui::epaint::PathStroke::new(arrow_width, color_y),
        )));
        painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
            vec![
                y_arrow_end,
                y_arrow_end + egui::Vec2::new(-arrow_head_width, arrow_head_width),
            ],
            egui::epaint::PathStroke::new(arrow_width, color_y),
        )));
        // Paint the +x arrow to ui.
        painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
            vec![
                screen_pos,
                x_arrow_end,
                x_arrow_end + egui::Vec2::new(-arrow_head_width, arrow_head_width),
            ],
            egui::epaint::PathStroke::new(arrow_width, color_x),
        )));
        painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
            vec![
                x_arrow_end,
                x_arrow_end + egui::Vec2::new(-arrow_head_width, -arrow_head_width),
            ],
            egui::epaint::PathStroke::new(arrow_width, color_x),
        )));
        delta
    }

    fn draw_handle(
        ui: &mut egui::Ui,
        screen_pos: egui::Pos2,
        screen_dir: egui::Vec2,
        handle_width: f32,
        handle_head_width: f32,
        color: egui::Color32,
        color_dragged: egui::Color32,
    ) -> egui::Vec2 {
        let mut delta = egui::Vec2::ZERO;
        let mut color = color;

        // Compute rect.
        let handle_head_rect = egui::Rect::from_center_size(
            screen_pos + screen_dir,
            egui::Vec2::splat(handle_head_width),
        );

        // Fetch input responses.
        let id = ui.next_auto_id();
        ui.skip_ahead_auto_ids(1);
        let response = ui.interact(handle_head_rect, id, egui::Sense::drag());
        if response.hovered() {
            color = color_dragged;
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            delta = response.drag_delta();
            color = color_dragged;
        }

        // Paint handle to ui.
        let painter = ui.painter();
        painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
            vec![screen_pos, screen_pos + screen_dir],
            egui::epaint::PathStroke::new(handle_width, color),
        )));
        painter.rect_filled(handle_head_rect, 0.0, color);

        delta
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

        let tick_this_frame = self.draw_engine_controller(ctx);
        self.draw_world_viewer(ctx);
        self.draw_body_creator(ctx);

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
